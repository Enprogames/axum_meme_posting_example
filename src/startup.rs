use crate::errors::AppError;
use aws_sdk_dynamodb::{
    operation::create_table::CreateTableError,
    types::{AttributeDefinition, BillingMode, KeySchemaElement, KeyType, ScalarAttributeType},
    Client as DynamoDbClient, error::SdkError as DynamoSdkError_CreateTable,
};
use aws_sdk_s3::{
    operation::create_bucket::CreateBucketError,
    types::{BucketLocationConstraint, CreateBucketConfiguration},
    Client as S3Client, error::SdkError as S3SdkError_CreateBucket,
};
use backoff::{future::retry, ExponentialBackoff};
use std::time::Duration;
use tracing::{error, info, warn};

// --- Retry Configuration ---

fn default_resource_backoff() -> ExponentialBackoff {
    ExponentialBackoff {
        initial_interval: Duration::from_millis(500),
        max_interval: Duration::from_secs(5),
        multiplier: 2.0,
        max_elapsed_time: Some(Duration::from_secs(15)),
        ..Default::default()
    }
}

// --- DynamoDB Initialization ---

fn is_dynamodb_create_error_retryable(err: &DynamoSdkError_CreateTable<CreateTableError>) -> bool {
    match err {
        DynamoSdkError_CreateTable::DispatchFailure(_) | DynamoSdkError_CreateTable::TimeoutError(_) => true,
        DynamoSdkError_CreateTable::ServiceError(se) => {
            !se.err().is_resource_in_use_exception()
        }
        _ => false,
    }
}

/// Attempts to create the DynamoDB table if it doesn't exist, applying retry logic.
// Added table_name parameter
async fn try_create_dynamodb_table(client: &DynamoDbClient, table_name: &str) -> Result<(), AppError> {
    let operation = || async {
        let attr_def = AttributeDefinition::builder()
            .attribute_name("meme_id")
            .attribute_type(ScalarAttributeType::S)
            .build()
            .map_err(|e| backoff::Error::permanent(DynamoSdkError_CreateTable::construction_failure(e)))?;

        let key_schema = KeySchemaElement::builder()
            .attribute_name("meme_id")
            .key_type(KeyType::Hash)
            .build()
            .map_err(|e| backoff::Error::permanent(DynamoSdkError_CreateTable::construction_failure(e)))?;

        client
            .create_table()
            .table_name(table_name) // Use parameter
            .attribute_definitions(attr_def)
            .key_schema(key_schema)
            .billing_mode(BillingMode::PayPerRequest)
            .send()
            .await
            .map_err(|sdk_error| {
                if is_dynamodb_create_error_retryable(&sdk_error) {
                    warn!(%table_name, error = %sdk_error, "Transient error creating DynamoDB table, retrying..."); // Use parameter in log
                    backoff::Error::transient(sdk_error)
                } else {
                    backoff::Error::permanent(sdk_error)
                }
            })
    };

    let result = retry(default_resource_backoff(), operation).await;

    match result {
        Ok(output) => {
            info!(%table_name, output = ?output, "DynamoDB table creation initiated/succeeded."); // Use parameter
            Ok(())
        }
        Err(sdk_error) => {
            // Pass table_name for context in error handling
            handle_final_dynamodb_error(sdk_error, table_name)
        }
    }
}

// Added table_name parameter
fn handle_final_dynamodb_error(sdk_error: DynamoSdkError_CreateTable<CreateTableError>, table_name: &str) -> Result<(), AppError> {
    if let Some(service_error) = sdk_error.as_service_error() {
        if service_error.is_resource_in_use_exception() {
            info!(%table_name, "DynamoDB table already exists."); // Use parameter
            Ok(())
        } else {
            let context = format!("Unrecoverable service error creating DynamoDB table '{}'", table_name); // Use parameter
            error!(error = ?service_error, %context);
            Err(AppError::InitError(format!("{}: {}", context, sdk_error)))
        }
    } else {
        let context = format!("Unrecoverable SDK error creating DynamoDB table '{}'", table_name); // Use parameter
        error!(error = %sdk_error, %context);
        Err(AppError::InitError(format!("{}: {}", context, sdk_error)))
    }
}


// --- S3 Initialization ---

// No changes needed in S3 retry logic itself for this refactor
fn is_s3_create_error_retryable(err: &S3SdkError_CreateBucket<CreateBucketError>) -> bool {
    match err {
        S3SdkError_CreateBucket::DispatchFailure(_) | S3SdkError_CreateBucket::TimeoutError(_) => true,
        S3SdkError_CreateBucket::ServiceError(se) => {
            let code = se.err().meta().code();
            !(code == Some("BucketAlreadyOwnedByYou") || code == Some("BucketAlreadyExists"))
        }
        _ => false,
    }
}

async fn try_create_s3_bucket(
    client: &S3Client,
    bucket_name: &str,
    region_str: &str,
) -> Result<(), AppError> {
    let operation = || async {
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

        create_bucket_req_builder
            .send()
            .await
            .map_err(|sdk_error| {
                if is_s3_create_error_retryable(&sdk_error) {
                    warn!(%bucket_name, error = %sdk_error, "Transient error creating S3 bucket, retrying...");
                    backoff::Error::transient(sdk_error)
                } else {
                    backoff::Error::permanent(sdk_error)
                }
            })
    };

    let result = retry(default_resource_backoff(), operation).await;

    match result {
        Ok(output) => {
            info!(%bucket_name, output = ?output, "S3 bucket creation initiated/succeeded.");
            Ok(())
        }
        Err(sdk_error) => {
             handle_final_s3_error(sdk_error, bucket_name)
        }
    }
}

 fn handle_final_s3_error(sdk_error: S3SdkError_CreateBucket<CreateBucketError>, bucket_name: &str) -> Result<(), AppError> {
     if let Some(service_error) = sdk_error.as_service_error() {
         let code = service_error.meta().code();
         if code == Some("BucketAlreadyOwnedByYou") || code == Some("BucketAlreadyExists") {
             info!(%bucket_name, "S3 bucket already exists.");
             Ok(())
         } else {
             let context = format!("Unrecoverable service error creating S3 bucket '{}'", bucket_name);
             error!(error = ?service_error, %context);
             Err(AppError::InitError(format!("{}: {}", context, sdk_error)))
         }
     } else {
         let context = format!("Unrecoverable SDK error creating S3 bucket '{}'", bucket_name);
         error!(error = %sdk_error, %context);
         Err(AppError::InitError(format!("{}: {}", context, sdk_error)))
     }
 }

// --- Main Initialization Function ---

/// Initializes required AWS resources (DynamoDB table, S3 bucket) during application startup.
/// Applies retry logic with exponential backoff for transient connection or service errors.
// Added table_name parameter
pub async fn init_resources(
    db_client: &DynamoDbClient,
    s3_client: &S3Client,
    table_name: &str, // Accept table_name from config
    bucket_name: &str,
    region_str: &str,
) -> Result<(), AppError> {
    info!("Initializing AWS resources...");

    // Pass table_name from config
    try_create_dynamodb_table(db_client, table_name).await?;
    try_create_s3_bucket(s3_client, bucket_name, region_str).await?;

    info!("AWS resource initialization complete.");
    Ok(())
}
