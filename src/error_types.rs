// src/error_types.rs
use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};
use aws_sdk_s3::error::SdkError;
use std::env;
use serde_json;
use anyhow;
use aws_smithy_types::error::operation::BuildError as SmithyBuildError;

#[derive(Debug)]
pub enum AppError {
    MissingFormField(String),
    MultipartError(axum::extract::multipart::MultipartError),
    // Removed: AwsS3Error(String),
    // Removed: AwsDynamoDbError(String),
    DatabaseError(anyhow::Error),
    NotFound(String),
    InternalServerError(String),
    IoError(std::io::Error),
    EnvVarError(env::VarError),
    AwsSdkError(String), // Generic catch-all for SDK errors
    InvalidInput(String),
    BuildError(String),
}

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
            // Removed AwsS3Error case
            // Removed AwsDynamoDbError case
            AppError::DatabaseError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database operation failed: {}", e),
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
                format!("Configuration error (environment variable): {}", e),
            ),
            AppError::AwsSdkError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AWS SDK error: {}", e),
            ),
            AppError::InvalidInput(msg) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid input: {}", msg),
            ),
            AppError::BuildError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to build AWS request: {}", e),
            ),
        };

        tracing::error!(error.message = %error_message, error.status = %status, "Responding with error");

        (status, Json(serde_json::json!({ "error": error_message }))).into_response()
    }
}

// Generic From<SdkError<E>> Implementation remains the same...
impl<E> From<SdkError<E>> for AppError
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(err: SdkError<E>) -> Self {
        AppError::AwsSdkError(format!("{}", err))
    }
}

// Other From implementations remain the same...
impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::MultipartError(err)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        tracing::error!(details = ?err, "Database operation failed");
        AppError::DatabaseError(err)
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

impl From<SmithyBuildError> for AppError {
     fn from(err: SmithyBuildError) -> Self {
         AppError::BuildError(format!("{}", err))
     }
}
