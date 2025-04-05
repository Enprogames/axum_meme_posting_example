// src/error_types.rs
use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};
use aws_sdk_s3::error::SdkError;
use std::env;
use tracing::log::error;
use serde_json;

// Define a custom error type for the application
#[derive(Debug)]
pub enum AppError {
    MissingFormField(String),
    MultipartError(axum::extract::multipart::MultipartError),
    AwsS3Error(String), // Can keep for specific internal mapping if desired
    AwsDynamoDbError(String), // Can keep for specific internal mapping if desired
    DatabaseError(anyhow::Error), // Catch-all for DB interaction errors
    NotFound(String),
    InternalServerError(String),
    IoError(std::io::Error),
    EnvVarError(env::VarError),
    AwsSdkError(String), // Added a specific variant for the generic SDK error
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
            // Keep specific variants if you map to them internally
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
                format!("Configuration error: {}", e),
            ),
            // Handle the generic AWS SDK error variant
            AppError::AwsSdkError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AWS SDK error: {}", e),
            ),
        };

        // Log the error for debugging purposes
        // Ensure tracing::error is used if you initialized tracing
        // Use log::error if using the `log` crate directly
        tracing::error!("Responding with status {}: {}", status, error_message); // Changed to tracing::error

        (status, Json(serde_json::json!({ "error": error_message }))).into_response()
    }
}

// --- Remove the conflicting specific From implementations ---
/*
// This conflicts with the generic one below
impl From<SdkError<aws_sdk_s3::operation::put_object::PutObjectError>> for AppError {
    fn from(err: SdkError<aws_sdk_s3::operation::put_object::PutObjectError>) -> Self {
        AppError::AwsS3Error(format!("S3 PutObject failed: {}", err))
    }
}

// This conflicts with the generic one below
impl From<SdkError<aws_sdk_s3::operation::create_bucket::CreateBucketError>> for AppError {
    fn from(err: SdkError<aws_sdk_s3::operation::create_bucket::CreateBucketError>) -> Self {
        AppError::AwsS3Error(format!("S3 CreateBucket failed: {}", err))
    }
}
*/

// --- Keep ONLY the Generic `From<SdkError<E>>` Implementation ---
impl<E> From<SdkError<E>> for AppError
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(err: SdkError<E>) -> Self {
        // You can add more specific checks here if needed:
        // For example:
        // if let Some(service_error) = err.as_service_error() {
        //     if service_error.is_some_specific_error_kind() {
        //         return AppError::SpecificVariant(...)
        //     }
        // }
        // Default generic conversion:
        AppError::AwsSdkError(format!("AWS SDK Error: {}", err)) // Use the new variant
    }
}


// --- Other From implementations remain the same ---

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::MultipartError(err)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        // Consider context - maybe distinguish DB errors more?
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