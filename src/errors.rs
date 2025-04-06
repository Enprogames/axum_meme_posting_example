use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;
use uuid::Uuid;

// --- Domain/Infrastructure Errors ---

#[derive(Error, Debug)]
pub enum RepoError {
    #[error("Meme metadata not found with ID: {0}")] // Clarify metadata
    NotFound(Uuid),
    #[error("Database backend error: {0}")]
    BackendError(#[from] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("File upload failed: {0}")]
    UploadFailed(String),
    #[error("File not found with key: {0}")]
    NotFound(String), // Keep this specific variant
    #[error("Storage backend error: {0}")]
    BackendError(#[from] anyhow::Error),
}

// --- Web Layer Error ---

#[derive(Error, Debug)]
pub enum AppError {
    // Input errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Missing form field: {0}")]
    MissingFormField(String),
    #[error("Error processing multipart form data: {0}")]
    MultipartError(#[from] axum::extract::multipart::MultipartError),
    #[error("Invalid meme ID format: {0}")]
    InvalidUuid(#[from] uuid::Error),

    // Not Found Errors
    #[error("Meme metadata not found with ID: {0}")]
    MemeNotFound(Uuid), // Specific for metadata
    #[error("Image file not found with key: {0}")]
    ImageNotFound(String), // Specific for image file

    // Domain/Service level errors
    #[error("Could not save meme data")]
    RepositoryError(#[source] RepoError),
    #[error("Could not perform file storage operation")]
    StorageError(#[source] StorageError), // Catch-all for other storage errors

    // Configuration / Startup errors
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Initialization error: {0}")]
    InitError(String),

    // Generic Internal Server Error
    #[error("Internal server error: {0}")]
    InternalServerError(String),
}

// --- Conversions from Domain Errors to AppError ---

impl From<RepoError> for AppError {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::NotFound(id) => AppError::MemeNotFound(id),
            // Map backend errors from repo -> generic repository error
            e @ RepoError::BackendError(_) => AppError::RepositoryError(e),
        }
    }
}

impl From<StorageError> for AppError {
    fn from(err: StorageError) -> Self {
        match err {
            // Map storage NotFound -> specific AppError ImageNotFound
            StorageError::NotFound(key) => AppError::ImageNotFound(key),
            // Map other storage errors -> generic storage error
            e => AppError::StorageError(e),
        }
    }
}

impl From<crate::config::ConfigError> for AppError { // ... unchanged ... }
// ... unchanged ...
    fn from(err: crate::config::ConfigError) -> Self {
        AppError::ConfigError(err.to_string())
    }
}


// --- Axum Response Implementation ---

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            // 4xx Client Errors
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::MissingFormField(field) => (StatusCode::BAD_REQUEST, format!("Missing form field: {}", field)),
            AppError::MultipartError(e) => (StatusCode::BAD_REQUEST, format!("Invalid multipart form data: {}", e)),
            AppError::InvalidUuid(e) => (StatusCode::BAD_REQUEST, format!("Invalid ID format: {}", e)),
            AppError::MemeNotFound(id) => (StatusCode::NOT_FOUND, format!("Meme metadata not found with ID: {}", id)),
            // --- Add specific 404 for image ---
            AppError::ImageNotFound(key) => (StatusCode::NOT_FOUND, format!("Image not found with key: {}", key)),
            // -----------------------------------

            // 5xx Server Errors
            AppError::RepositoryError(e) => {
                tracing::error!(error.source = ?e, "Repository error occurred");
                (StatusCode::INTERNAL_SERVER_ERROR, "Database operation failed".to_string())
            },
            AppError::StorageError(e) => { // This now catches non-NotFound storage errors
                tracing::error!(error.source = ?e, "Storage error occurred");
                (StatusCode::INTERNAL_SERVER_ERROR, "File storage operation failed".to_string())
            },
            AppError::ConfigError(msg) => { // ... unchanged ... }
                tracing::error!("Configuration error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Server configuration error".to_string())
            },
            AppError::InitError(msg) => { // ... unchanged ... }
                tracing::error!("Initialization error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Server initialization error".to_string())
            }
            AppError::InternalServerError(msg) => { // ... unchanged ... }
                tracing::error!("Internal server error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "An internal server error occurred".to_string())
            }
        };

        tracing::error!(error.message=%error_message, error.detail=%self, "Responding with error");

        let body = Json(serde_json::json!({ "error": error_message }));
        (status, body).into_response()
    }
}
