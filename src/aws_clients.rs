use aws_config::SdkConfig;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::Client as S3Client;
// Import Credentials needed for static credentials
use aws_credential_types::Credentials;

/// Loads the AWS SDK configuration.
///
/// The configuration points to a local endpoint (e.g. Localstack on http://localhost:4566)
/// and uses the "ca-central-1" region. Also enables dummy credentials for LocalStack.
pub async fn load_config() -> SdkConfig {
    aws_config::defaults(aws_config::BehaviorVersion::latest())
        // Setting endpoint_url tells the SDK to target LocalStack
        .endpoint_url("http://localhost:4566")
        // Provide explicit dummy credentials for LocalStack interaction
        .credentials_provider(Credentials::new(
            "test", // access_key_id
            "test", // secret_access_key
            None,   // session_token
            None,   // expiry
            "StaticCredentials", // provider_name
        ))
        // Ensure the region matches LocalStack and your intended region
        .region(aws_config::Region::new("ca-central-1"))
        .load()
        .await
}

/// Creates and returns a DynamoDB client configured for LocalStack.
pub async fn create_dynamodb_client() -> DynamoDbClient {
    let config = load_config().await;
    DynamoDbClient::new(&config)
}

/// Creates and returns an S3 client configured for LocalStack.
pub async fn create_s3_client() -> S3Client {
    let shared_config = load_config().await; // Load the base SdkConfig

    // Create S3 specific config FROM the shared config
    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .force_path_style(true) // Apply S3 specific settings
        .build();

    // Create the S3 client using the S3 specific config
    S3Client::from_conf(s3_config)
}
