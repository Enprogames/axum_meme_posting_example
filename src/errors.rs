use crate::config;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;
use tracing;
use uuid::Uuid;

// --- Domain/Infrastructure Errors ---

#[derive(Error, Debug)]
pub enum RepoError {
    #[error("Meme metadata not found with ID: {0}")] // Clarify metadata
    NotFound(Uuid),
    #[error("Database backend error: {0}")]
    BackendError(#[from] anyhow::Error), // Allows easy conversion from SDK/other errors via context()
    #[error("Data corruption detected: {0}")] // Error for unparseable data from DB
    DataCorruption(String),
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("File upload failed: {0}")]
    UploadFailed(String), // Could be more specific if needed
    #[error("File not found with key: {0}")]
    NotFound(String), // Specific variant for file not found
    #[error("Storage backend error: {0}")]
    BackendError(#[from] anyhow::Error), // Catch-all for SDK/backend issues
}

// --- Web Layer Error ---

#[derive(Error, Debug)]
pub enum AppError {
    // Input errors (4xx)
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Missing form field: {0}")]
    MissingFormField(String),
    #[error("Error processing multipart form data: {0}")]
    MultipartError(#[from] axum::extract::multipart::MultipartError),
    #[error("Invalid meme ID format: {0}")]
    InvalidUuid(#[from] uuid::Error),

    // Not Found Errors (404)
    #[error("Meme metadata not found with ID: {0}")]
    MemeNotFound(Uuid), // Specific for metadata from repo
    #[error("Image file not found with key: {0}")]
    ImageNotFound(String), // Specific for image file from storage

    // Domain/Service level errors (5xx)
    #[error("Could not process meme data")] // User-friendly message
    RepositoryError(#[source] RepoError), // Wraps underlying RepoError
    #[error("Could not perform file storage operation")] // User-friendly message
    StorageError(#[source] StorageError), // Wraps underlying StorageError

    // Configuration / Startup errors (5xx)
    #[error("Configuration error: {0}")]
    ConfigError(String), // Keep String representation for simplicity
    #[error("Initialization error: {0}")] // Includes SDK config errors now via From trait
    InitError(String),

    // Generic Internal Server Error (5xx)
    #[error("Internal server error: {0}")]
    InternalServerError(String),
}

// --- Conversions from Domain Errors to AppError ---

impl From<RepoError> for AppError {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::NotFound(id) => AppError::MemeNotFound(id),
            // Map DataCorruption to the generic RepositoryError for handling
            e @ RepoError::DataCorruption(_) => {
                 tracing::error!(error.source = ?e, "Repository data corruption occurred");
                 AppError::RepositoryError(e) // Wrap the specific error
            }
            // Map other backend errors from repo -> generic repository error
            e @ RepoError::BackendError(_) => AppError::RepositoryError(e), // Use '@' binding
        }
    }
}

impl From<StorageError> for AppError {
    fn from(err: StorageError) -> Self {
        match err {
            // Map storage NotFound -> specific AppError ImageNotFound
            StorageError::NotFound(key) => AppError::ImageNotFound(key),
            // Map other storage errors (UploadFailed, BackendError) -> generic storage error
            e => AppError::StorageError(e), // Wrap the specific error
        }
    }
}

// Convert configuration errors
impl From<config::ConfigError> for AppError {
    fn from(err: config::ConfigError) -> Self {
        // Log the specific config error cause here for server visibility
        tracing::error!(error.source = ?err, "Configuration error");
        AppError::ConfigError(err.to_string()) // Return a user-friendly summary
    }
}

// --- Axum Response Implementation ---

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            // 4xx Client Errors
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::MissingFormField(field) => {
                (StatusCode::BAD_REQUEST, format!("Missing form field: {}", field))
            }
            AppError::MultipartError(e) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid multipart form data: {}", e),
            ),
            AppError::InvalidUuid(e) => (StatusCode::BAD_REQUEST, format!("Invalid ID format: {}", e)),
            AppError::MemeNotFound(id) => (
                StatusCode::NOT_FOUND,
                format!("Meme metadata not found with ID: {}", id),
            ),
            AppError::ImageNotFound(key) => {
                (StatusCode::NOT_FOUND, format!("Image not found with key: {}", key))
            }

            // 5xx Server Errors
            AppError::RepositoryError(e) => {
                // Log the specific underlying RepoError cause
                tracing::error!(error.source = ?e, "Repository error occurred");
                // Return generic message to client
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database operation failed".to_string(),
                )
            }
            AppError::StorageError(e) => {
                // Log the specific underlying StorageError cause
                tracing::error!(error.source = ?e, "Storage error occurred");
                 // Return generic message to client
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "File storage operation failed".to_string(),
                )
            }
            AppError::ConfigError(_msg) => {
                // Config error was already logged in From trait impl
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Server configuration error".to_string(),
                )
            }
            AppError::InitError(msg) => {
                tracing::error!("Initialization error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Server initialization error".to_string(),
                )
            }
            AppError::InternalServerError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal server error occurred".to_string(),
                )
            }
        };

        // Log the final error response details (excluding sensitive source details logged above)
        tracing::warn!(status = %status, error.message = %error_message, "Responding with error");

        // Format the response body as JSON
        let body = Json(serde_json::json!({ "error": error_message }));
        (status, body).into_response()
    }
}
