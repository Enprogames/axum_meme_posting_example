use crate::config::Config;
use crate::errors::AppError;
use aws_config::{Region, BehaviorVersion, SdkConfig};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::Client as S3Client;
use tracing;

// Creates the base AWS SDK configuration based on application config.
// Reads region and optional endpoint URL from `Config`.
// Uses the default credential provider chain (which reads env vars, profiles, etc.).
pub async fn create_sdk_config(config: &Config) -> Result<SdkConfig, AppError> {
    let region = Region::new(config.aws_region.clone());
    tracing::info!(sdk_region = %config.aws_region, "Setting SDK region");

    let mut config_loader = aws_config::defaults(BehaviorVersion::latest())
        .region(region); 

    if let Some(endpoint_url) = &config.localstack_endpoint {
        tracing::info!("Using localstack endpoint override: {}", endpoint_url);
        config_loader = config_loader.endpoint_url(endpoint_url);
    } else {
        tracing::info!("Using default AWS endpoints and credential resolution.");
    }

    // Load the configuration.
    let sdk_config_result = config_loader.load().await;

    Ok(sdk_config_result)
}

// Creates a DynamoDB client from a shared SdkConfig.
pub fn create_dynamodb_client(sdk_config: &SdkConfig) -> DynamoDbClient {
    DynamoDbClient::new(sdk_config)
}

// Creates an S3 client from a shared SdkConfig.
pub fn create_s3_client(sdk_config: &SdkConfig) -> S3Client {
    let s3_config_builder = aws_sdk_s3::config::Builder::from(sdk_config)
        .force_path_style(true);
    let s3_config = s3_config_builder.build();
    S3Client::from_conf(s3_config)
}