use crate::errors::AppError;
use aws_sdk_dynamodb::{
    types::{AttributeDefinition, BillingMode, KeySchemaElement, KeyType, ScalarAttributeType},
    Client as DynamoDbClient, error::SdkError as DynamoSdkError,
};
use aws_sdk_s3::{
    types::{BucketLocationConstraint, CreateBucketConfiguration},
    Client as S3Client, error::SdkError as S3SdkError,
};
use tracing;

const MEMES_TABLE: &str = "memes";

/// Creates the DynamoDB table if it doesn't exist.
async fn create_dynamodb_table_if_not_exists(client: &DynamoDbClient) -> Result<(), AppError> {
    let result = client
        .create_table()
        .table_name(MEMES_TABLE)
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("meme_id")
                .attribute_type(ScalarAttributeType::S)
                .build()
                .map_err(|e| AppError::InitError(format!("Failed to build attribute definition: {}", e)))?
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("meme_id")
                .key_type(KeyType::Hash)
                .build()
                .map_err(|e| AppError::InitError(format!("Failed to build key schema: {}", e)))?
        )
        .billing_mode(BillingMode::PayPerRequest)
        .send()
        .await;
    match result {
        Ok(_) => {
            tracing::info!("Startup: Table '{}' created successfully or setup initiated.", MEMES_TABLE);
            Ok(())
        }
        Err(e) => {
            if let DynamoSdkError::ServiceError(service_err) = &e {
                if service_err.err().is_resource_in_use_exception() {
                    tracing::info!("Startup: Table '{}' already exists, no action needed.", MEMES_TABLE);
                    Ok(())
                } else {
                    let context = format!("Startup: Service error creating DynamoDB table '{}'", MEMES_TABLE);
                    tracing::error!("{}: {:?}", context, service_err);
                    Err(AppError::InitError(format!("{}: {}", context, e)))
                }
            } else {
                let context = format!("Startup: SDK error creating DynamoDB table '{}'", MEMES_TABLE);
                 tracing::error!("{}: {}", context, e);
                 Err(AppError::InitError(format!("{}: {}", context, e)))
            }
        }
    }
}


/// Ensures the S3 bucket exists, creating it with the correct location constraint if needed.
async fn ensure_s3_bucket_exists(client: &S3Client, bucket_name: &str, region_str: &str) -> Result<(), AppError> {
    let bucket_config = if region_str != "us-east-1" {
        Some(
            CreateBucketConfiguration::builder()
                .location_constraint(BucketLocationConstraint::from(region_str))
                .build(),
        )
    } else {
        None
    };

    let mut create_bucket_req_builder = client.create_bucket().bucket(bucket_name);
    if let Some(config) = bucket_config {
        create_bucket_req_builder = create_bucket_req_builder.create_bucket_configuration(config);
    }

    match create_bucket_req_builder.send().await {
        Ok(_) => {
            tracing::info!("Startup: S3 bucket '{}' created or already exists.", bucket_name);
            Ok(())
        }
        Err(sdk_err) => { // Use sdk_err as the variable name here
             if let S3SdkError::ServiceError(service_err) = &sdk_err {
                let code = service_err.err().meta().code();
                if code == Some("BucketAlreadyOwnedByYou") || code == Some("BucketAlreadyExists") {
                    tracing::info!("Startup: S3 bucket '{}' already exists.", bucket_name);
                    Ok(())
                } else {
                    let context = format!("Startup: Service error creating S3 bucket '{}'", bucket_name);
                    tracing::error!("{}: {:?}", context, service_err);
                    Err(AppError::InitError(format!("{}: {}", context, sdk_err)))
                }
            } else {
                let context = format!("Startup: SDK error creating S3 bucket '{}'", bucket_name);
                 tracing::error!("{}: {}", context, sdk_err);
                 Err(AppError::InitError(format!("{}: {}", context, sdk_err)))
            }
        }
    }
}


/// Initializes required AWS resources (DynamoDB table, S3 bucket).
pub async fn init_resources(
    db_client: &DynamoDbClient,
    s3_client: &S3Client,
    bucket_name: &str,
    region_str: &str,
) -> Result<(), AppError> {
    tracing::info!("Startup: Initializing AWS resources...");
    create_dynamodb_table_if_not_exists(db_client).await?;
    ensure_s3_bucket_exists(s3_client, bucket_name, region_str).await?;
    tracing::info!("Startup: AWS resource initialization complete.");
    Ok(())
}
