use std::{env, net::SocketAddr, str::FromStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingVar(String),
    #[error("Invalid environment variable format for {0}: {1}")]
    InvalidVar(String, String),
    #[error(transparent)]
    DotEnvError(#[from] dotenvy::Error),
}

#[derive(Clone, Debug)] // Clone needed if passed around, Debug for logging
pub struct Config {
    pub bind_address: SocketAddr,
    pub meme_bucket_name: String,
    // Store region as string for simplicity here, aws_clients can convert
    pub aws_region: String,
    // Optional endpoint for LocalStack
    pub localstack_endpoint: Option<String>,
}

impl Config {
    /// Loads configuration from environment variables.
    pub fn load() -> Result<Self, ConfigError> {
        // Load .env file if present (ignores errors, relies on env vars otherwise)
        dotenvy::dotenv().ok();

        let bind_address_str = env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        let bind_address = SocketAddr::from_str(&bind_address_str)
            .map_err(|e| ConfigError::InvalidVar("BIND_ADDRESS".into(), e.to_string()))?;

        let meme_bucket_name = env::var("MEME_BUCKET_NAME")
            .map_err(|_| ConfigError::MissingVar("MEME_BUCKET_NAME".into()))?;

        let aws_region = env::var("AWS_DEFAULT_REGION")
            .unwrap_or_else(|_| "ca-central-1".to_string());

        // Allow overriding endpoint for localstack/testing
        let localstack_endpoint = env::var("AWS_ENDPOINT_URL").ok(); // Optional

        Ok(Config {
            bind_address,
            meme_bucket_name,
            aws_region,
            localstack_endpoint,
        })
    }
}
