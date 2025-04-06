use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error; // Use thiserror for cleaner error definitions
use uuid::Uuid;

// --- Domain/Infrastructure Errors ---

#[derive(Error, Debug)]
pub enum RepoError {
    #[error("Meme not found with ID: {0}")]
    NotFound(Uuid), // More specific than just string

    #[error("Database backend error: {0}")]
    BackendError(#[from] anyhow::Error), // Wrap Anyhow errors from DB layer

    // Example: Could add constraint violation errors etc. if needed
    // #[error("Duplicate meme ID: {0}")]
    // Duplicate(Uuid),
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("File upload failed: {0}")]
    UploadFailed(String), // Pass specific reason

    #[error("File not found with key: {0}")]
    NotFound(String),

    #[error("Storage backend error: {0}")]
    BackendError(#[from] anyhow::Error), // Wrap Anyhow errors from Storage layer
}

// --- Web Layer Error ---

#[derive(Error, Debug)]
pub enum AppError {
    // Input validation / request parsing errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Missing form field: {0}")]
    MissingFormField(String),
    #[error("Error processing multipart form data: {0}")]
    MultipartError(#[from] axum::extract::multipart::MultipartError),
    #[error("Invalid meme ID format: {0}")]
    InvalidUuid(#[from] uuid::Error),

    // Domain/Service level errors (mapped from RepoError/StorageError)
    #[error("Meme not found with ID: {0}")]
    MemeNotFound(Uuid),
    #[error("Could not save meme data")]
    RepositoryError(#[source] RepoError), // Source allows seeing underlying RepoError
    #[error("Could not perform file storage operation")]
    StorageError(#[source] StorageError), // Source allows seeing underlying StorageError

    // Configuration / Startup errors
    #[error("Configuration error: {0}")]
    ConfigError(String), // Keep simple string for now
    #[error("Initialization error: {0}")]
    InitError(String),

    // Generic Internal Server Error
    #[error("Internal server error: {0}")]
    InternalServerError(String), // Catch-all or specific internal issues
}

// --- Conversions from Domain Errors to AppError ---

impl From<RepoError> for AppError {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::NotFound(id) => AppError::MemeNotFound(id),
            // Other RepoError variants could be mapped specifically if needed
            e @ RepoError::BackendError(_) => AppError::RepositoryError(e),
        }
    }
}

impl From<StorageError> for AppError {
    fn from(err: StorageError) -> Self {
        // Map all storage errors generally for now
        AppError::StorageError(err)
    }
}

// Add From impl for ConfigError if Config::load can fail in main
impl From<crate::config::ConfigError> for AppError {
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
            AppError::MemeNotFound(id) => (StatusCode::NOT_FOUND, format!("Meme not found with ID: {}", id)),

            // 5xx Server Errors
            AppError::RepositoryError(e) => {
                tracing::error!(error.source = ?e, "Repository error occurred");
                (StatusCode::INTERNAL_SERVER_ERROR, "Database operation failed".to_string())
            },
            AppError::StorageError(e) => {
                tracing::error!(error.source = ?e, "Storage error occurred");
                (StatusCode::INTERNAL_SERVER_ERROR, "File storage operation failed".to_string())
            },
            AppError::ConfigError(msg) => {
                tracing::error!("Configuration error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Server configuration error".to_string())
            },
            AppError::InitError(msg) => {
                tracing::error!("Initialization error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Server initialization error".to_string())
            }
            AppError::InternalServerError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "An internal server error occurred".to_string())
            }
        };

        // Log the specific error variant and message
        tracing::error!(error.message=%error_message, error.detail=%self, "Responding with error");

        // Build JSON response
        let body = Json(serde_json::json!({ "error": error_message }));
        (status, body).into_response()
    }
}

// Helper macro for creating internal server errors with context
macro_rules! internal_error {
    ($err:expr) => {
        AppError::InternalServerError(format!("{}: {}", std::line!(), $err))
    };
     ($fmt:expr, $($arg:tt)*) => {
        AppError::InternalServerError(format!(concat!("{}: ", $fmt), std::line!(), $($arg)*))
    };
}
// Make macro available in other modules
pub(crate) use internal_error;
