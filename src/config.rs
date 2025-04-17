use std::{env, net::SocketAddr, str::FromStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingVar(String),
    #[error("Invalid environment variable format for {0}: {1}")]
    InvalidVar(String, String),
    #[error("Failed to load .env file: {0}")]
    DotEnvError(#[from] dotenvy::Error),
}

#[derive(Clone, Debug)] // Clone needed for AppState, Debug for logging
pub struct Config {
    pub bind_address: SocketAddr,
    pub meme_bucket_name: String,
    pub dynamodb_table_name: String, // Added
    pub aws_region: String,
    pub localstack_endpoint: Option<String>,
}

impl Config {
    /// Loads configuration from environment variables.
    /// Reads from a .env file if present.
    pub fn load() -> Result<Self, ConfigError> {
        // Attempt to load .env file, ignore if not found
        dotenvy::dotenv().ok();

        // --- Application Specific Config ---
        let bind_address_str = env::var("APP_SERVER_ADDRESS")
            .unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        let bind_address = SocketAddr::from_str(&bind_address_str)
            .map_err(|e| ConfigError::InvalidVar("APP_SERVER_ADDRESS".into(), e.to_string()))?;

        // Required variables - return specific error if missing
        let meme_bucket_name = env::var("APP_S3_BUCKET_NAME")
            .map_err(|_| ConfigError::MissingVar("APP_S3_BUCKET_NAME".into()))?;

        let dynamodb_table_name = env::var("APP_DYNAMODB_TABLE_NAME")
            .map_err(|_| ConfigError::MissingVar("APP_DYNAMODB_TABLE_NAME".into()))?;


        // --- AWS Related Config ---
        // Use standard AWS SDK environment variables
        // Prefer AWS_REGION if set, fallback to AWS_DEFAULT_REGION, then to hardcoded default
        let aws_region = env::var("AWS_REGION")
            .or_else(|_| env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "ca-central-1".to_string()); // Default region

        // Optional override for LocalStack/testing
        let localstack_endpoint = env::var("AWS_ENDPOINT_URL").ok();

        info!(
            bind_address = %bind_address,
            bucket_name = %meme_bucket_name,
            table_name = %dynamodb_table_name,
            region = %aws_region,
            endpoint_url = ?localstack_endpoint,
            "Configuration loaded"
        ); // Added info log

        Ok(Config {
            bind_address,
            meme_bucket_name,
            dynamodb_table_name, // Include new field
            aws_region,
            localstack_endpoint,
        })
    }
}

use tracing::info;
