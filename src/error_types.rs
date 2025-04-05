use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};
use aws_sdk_s3::error::SdkError;
use std::env;
// use tracing::log::error; // <-- Remove this unused import
use serde_json;
use anyhow; // Keep anyhow if used in AppError::DatabaseError

// Define a custom error type for the application
#[derive(Debug)]
pub enum AppError {
    MissingFormField(String),
    MultipartError(axum::extract::multipart::MultipartError),
    AwsS3Error(String),
    AwsDynamoDbError(String),
    DatabaseError(anyhow::Error), // Keep using anyhow::Error here
    NotFound(String),
    InternalServerError(String),
    IoError(std::io::Error),
    EnvVarError(env::VarError),
    AwsSdkError(String),
}

// Implement IntoResponse for AppError to convert errors into HTTP responses
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::MissingFormField(field) => (
                StatusCode::BAD_REQUEST,
                format!("Missing form field: {}", field),
            ),
            AppError::MultipartError(e) => (
                StatusCode::BAD_REQUEST,
                format!("Error processing multipart form: {}", e),
            ),
            AppError::AwsS3Error(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AWS S3 error: {}", e),
            ),
            AppError::AwsDynamoDbError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AWS DynamoDB error: {}", e),
            ),
            AppError::DatabaseError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database operation failed: {}", e), // anyhow::Error Display impl used here
            ),
            AppError::NotFound(item) => (StatusCode::NOT_FOUND, format!("{} not found", item)),
            AppError::InternalServerError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal server error: {}", msg),
            ),
            AppError::IoError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("IO error: {}", e),
            ),
            AppError::EnvVarError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Configuration error: {}", e),
            ),
            AppError::AwsSdkError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AWS SDK error: {}", e),
            ),
        };

        tracing::error!("Responding with status {}: {}", status, error_message);

        (status, Json(serde_json::json!({ "error": error_message }))).into_response()
    }
}

// Generic From<SdkError<E>> Implementation
impl<E> From<SdkError<E>> for AppError
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(err: SdkError<E>) -> Self {
        AppError::AwsSdkError(format!("AWS SDK Error: {}", err))
    }
}

// Other From implementations
impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::MultipartError(err)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::DatabaseError(err) // This converts anyhow results from db.rs
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err)
    }
}

impl From<env::VarError> for AppError {
    fn from(err: env::VarError) -> Self {
        AppError::EnvVarError(err)
    }
}