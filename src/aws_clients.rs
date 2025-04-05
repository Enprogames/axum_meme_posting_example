use aws_config::SdkConfig;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::Client as S3Client;

/// Loads the AWS SDK configuration.
/// 
/// The configuration points to a local endpoint (e.g. Localstack on http://localhost:4566)
/// and uses the "ca-central-1" region.
pub async fn load_config() -> SdkConfig {
    aws_config::from_env()
        .endpoint_url("http://localhost:4566")
        .region("ca-central-1")
        .load()
        .await
}

/// Creates and returns a DynamoDB client.
pub async fn create_dynamodb_client() -> DynamoDbClient {
    DynamoDbClient::new(&load_config().await)
}

/// Creates and returns an S3 client.
pub async fn create_s3_client() -> S3Client {
    S3Client::new(&load_config().await)
}
