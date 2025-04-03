use aws_config::SdkConfig;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::Client as S3Client;

pub async fn load_config() -> SdkConfig {
    aws_config::from_env()
        .endpoint_url("http://localhost:4566")
        .region("us-east-1")
        .load()
        .await
}

pub async fn create_dynamodb_client() -> DynamoDbClient {
    DynamoDbClient::new(&load_config().await)
}

pub async fn create_s3_client() -> S3Client {
    S3Client::new(&load_config().await)
}
