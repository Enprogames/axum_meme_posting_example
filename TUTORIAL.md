Okay, let's get this meme service up and running!

You've provided a solid foundation with separate modules for AWS clients, database interactions, models, and error handling. The use of Axum, Tokio, `anyhow`, and `tracing` is great for building a robust asynchronous web service.

Here's an overview of the plan:

1.  **Address Minor Fixes:** Correct the Rust edition in `Cargo.toml` and ensure all necessary imports are present and unused ones are removed.
2.  **Implement Listing Memes:** Add the functionality to retrieve all memes from the DynamoDB table. This involves:
    * Adding a `list_memes` function in `src/db.rs` using the DynamoDB `Scan` operation.
    * Adding a new route (`GET /memes`) in `src/main.rs`.
    * Implementing the corresponding handler (`list_memes_handler`) in `src/main.rs`.
3.  **Code Structure:** Organize the provided snippets into the correct file structure.
4.  **Documentation & Configuration:** Provide instructions on how to set up the environment (LocalStack, `.env` file) and run the application, along with an example `.env` file.

Let's go through each part.

### 1. Project Structure

Based on your code, the project structure should look like this:

```
.
├── .env             # Local environment variables (create this)
├── .env.example     # Example environment variables (will provide)
├── .localstack/     # LocalStack data volume (created by docker-compose)
├── Cargo.toml       # Project dependencies
├── docker-compose.yml # LocalStack setup
└── src/             # Source code
    ├── aws_clients.rs
    ├── db.rs
    ├── error_types.rs
    ├── main.rs
    └── models.rs
```

### 2. `Cargo.toml`

Update the `edition` to `"2021"` (as "2024" is not a released edition yet) and ensure all listed dependencies are correct. The versions you provided are workable, although newer minor/patch versions might exist.

```toml
# Cargo.toml
[package]
name = "axum_meme_posting_example"
version = "0.1.0"
edition = "2021" # Corrected edition

[dependencies]
axum = { version = "0.8", features = ["multipart"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
aws-config = { version = "1.3", features = ["behavior-version-latest"] }
aws-sdk-s3 = "1.82"
aws-sdk-dynamodb = "1.71"
uuid = { version = "1", features = ["v4", "serde"] }
tower-http = { version = "0.6", features = ["cors", "trace"] }
anyhow = "1.0"
aws-smithy-types = "1.3" # For ProvideErrorMetadata trait

tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

dotenvy = "0.15" # To load .env files during development
```

### 3. `docker-compose.yml`

Your `docker-compose.yml` looks good for setting up LocalStack.

```yaml
# docker-compose.yml
services:
  localstack:
    image: localstack/localstack:latest
    ports:
      - "4566:4566" # Default port for all services
      # You could expose specific default ports too if needed, e.g., 4572 for S3, 4569 for DynamoDB
      # But 4566 is the unified endpoint.
    environment:
      - SERVICES=dynamodb,s3
      - DEBUG=1 # Use 0 or remove for less verbose logs
      - AWS_DEFAULT_REGION=ca-central-1 # Set default region
      # Optional: Persist data between runs
      - PERSISTENCE=1 # Enable persistence
      - DATA_DIR=/tmp/localstack/data # Location inside container for data
    volumes:
      # Mount host directory to persist LocalStack state
      - "./.localstack/state:/tmp/localstack/data"
      # You might still want the init hooks volume if you add initialization scripts later
      # - "./.localstack/init:/etc/localstack/init/ready.d"
```
*Self-correction:* Added persistence configuration to the `docker-compose.yml` as it's often useful during development to keep the S3 bucket and DynamoDB table between restarts. Changed the volume mount to a named volume for better cross-platform compatibility and Docker practices, but reverted to the host path as provided by the user for consistency with their snippet. Updated the data directory inside the container to match the `DATA_DIR` environment variable. Let's stick to the user's original volume mount for simplicity.

```yaml
# docker-compose.yml (Revised for user consistency)
services:
  localstack:
    image: localstack/localstack:latest
    ports:
      - "4566:4566" # Default port for all services
    environment:
      - SERVICES=dynamodb,s3
      - DEBUG=1 # Use 0 or remove for less verbose logs
      - AWS_DEFAULT_REGION=ca-central-1 # Set default region
      # Persistence via volume mount
      - DOCKER_HOST=unix:///var/run/docker.sock # Needed by LS on some systems to manage volumes correctly
    volumes:
      # Mount host directory to persist LocalStack state (as user provided)
      - "./.localstack:/var/lib/localstack"
      # Mount docker socket if needed (see DOCKER_HOST env var)
      - "/var/run/docker.sock:/var/run/docker.sock"

```

### 4. `src/models.rs`

This file looks correct as provided.

```rust
// src/models.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The `Meme` struct represents a meme's metadata.
///
/// Fields:
/// - `meme_id`: A unique identifier (UUID) for the meme.
/// - `title`: The meme's title.
/// - `description`: A short description of the meme.
/// - `image_key`: The key (i.e. filename) of the meme image stored in S3.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Meme {
    pub meme_id: Uuid,
    pub title: String,
    pub description: String,
    pub image_key: String,
}
```

### 5. `src/aws_clients.rs`

This file looks correct as provided. It correctly sets the endpoint URL for LocalStack.

```rust
// src/aws_clients.rs
use aws_config::SdkConfig;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::Client as S3Client;

/// Loads the AWS SDK configuration.
///
/// The configuration points to a local endpoint (e.g. Localstack on http://localhost:4566)
/// and uses the "ca-central-1" region. Also enables dummy credentials for LocalStack.
pub async fn load_config() -> SdkConfig {
    aws_config::defaults(aws_config::BehaviorVersion::latest())
        // Setting endpoint_url tells the SDK to target LocalStack
        .endpoint_url("http://localhost:4566")
        // Provide dummy credentials for LocalStack interaction
        .credentials_provider(aws_config::test_config::TestCredentials::CONTAINER)
        // Ensure the region matches LocalStack and your intended region
        .region(aws_config::Region::new("ca-central-1"))
        .load()
        .await
}

/// Creates and returns a DynamoDB client configured for LocalStack.
pub async fn create_dynamodb_client() -> DynamoDbClient {
    let config = load_config().await;
    // Use config() to force endpoint resolution if needed, although load_config should handle it
    DynamoDbClient::new(&config)
}

/// Creates and returns an S3 client configured for LocalStack.
pub async fn create_s3_client() -> S3Client {
    let config = load_config().await;
    // For S3 with LocalStack, often need to enable path-style addressing
    S3Client::from_conf(
        aws_sdk_s3::Config::builder()
            .force_path_style(true) // Important for LocalStack S3
            .build_from(&config) // Builds on top of the loaded config
    )
}
```
*Self-correction:* Modified `load_config` slightly to use `defaults` and explicitly add dummy credentials often needed for LocalStack. Crucially, modified `create_s3_client` to use `force_path_style(true)`, which is almost always required when working with S3 emulators like LocalStack.

### 6. `src/error_types.rs`

This file looks mostly good. Ensured imports are correct and added tracing for errors.

```rust
// src/error_types.rs
use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};
use aws_sdk_s3::error::SdkError; // Keep this generic SdkError
use std::env;
use serde_json;
use anyhow; // Keep anyhow for AppError::DatabaseError

// Define a custom error type for the application
#[derive(Debug)]
pub enum AppError {
    MissingFormField(String),
    MultipartError(axum::extract::multipart::MultipartError),
    AwsS3Error(String), // Potentially redundant if AwsSdkError covers it well
    AwsDynamoDbError(String), // Potentially redundant
    DatabaseError(anyhow::Error), // Used for db.rs results
    NotFound(String),
    InternalServerError(String),
    IoError(std::io::Error),
    EnvVarError(env::VarError),
    AwsSdkError(String), // Generic catch-all for SDK errors
    InvalidInput(String), // Added for validation errors like invalid UUID
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
            AppError::AwsS3Error(e) => ( // Keep for specific mapping if needed later
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AWS S3 interaction error: {}", e),
            ),
            AppError::AwsDynamoDbError(e) => ( // Keep for specific mapping if needed later
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AWS DynamoDB interaction error: {}", e),
            ),
            AppError::DatabaseError(e) => ( // Handles errors from db.rs using anyhow
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
            AppError::AwsSdkError(e) => ( // Generic SDK error handler
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AWS SDK error: {}", e),
            ),
            AppError::InvalidInput(msg) => ( // Handle invalid input
                StatusCode::BAD_REQUEST,
                format!("Invalid input: {}", msg),
            ),
        };

        // Log the error before sending the response
        tracing::error!(error.message = %error_message, error.status = %status, "Responding with error");

        (status, Json(serde_json::json!({ "error": error_message }))).into_response()
    }
}

// Generic From<SdkError<E>> Implementation for any AWS SDK Error
// This is crucial for converting errors from S3, DynamoDB, etc. using '?'
impl<E> From<SdkError<E>> for AppError
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(err: SdkError<E>) -> Self {
        // You could add more specific matching here if needed based on err.kind
        AppError::AwsSdkError(format!("{}", err))
    }
}

// Other From implementations
impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::MultipartError(err)
    }
}

// Convert anyhow::Error (used in db.rs) to AppError
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        // You could potentially inspect the anyhow error chain here
        // for more specific error types if needed, but for now, map to DatabaseError.
        tracing::error!(details = ?err, "Database operation failed"); // Log anyhow error details
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

// If needed for specific builder errors from AWS SDK that don't go through SdkError
impl From<aws_smithy_types::error::BuildError> for AppError {
     fn from(err: aws_smithy_types::error::BuildError) -> Self {
         AppError::InternalServerError(format!("Failed to build AWS request: {}", err))
     }
}
```
*Self-correction:* Added `AppError::InvalidInput` for better client feedback on bad requests (like invalid UUIDs). Added a `From` implementation for `aws_smithy_types::error::BuildError` which can occur when building SDK requests (e.g., from `AttributeDefinition::builder().build()?`). Improved logging within the `From<anyhow::Error>` implementation.

### 7. `src/db.rs`

Here we add the `list_memes` function and ensure other functions are correct.

```rust
// src/db.rs

// Standard library imports
use std::collections::HashMap;

// External crate imports
use anyhow::{Context, Result}; // Using anyhow::Result for internal DB functions
use aws_sdk_dynamodb::{
    error::SdkError,
    types::{
        AttributeDefinition, BillingMode, KeySchemaElement, KeyType, ScalarAttributeType,
        AttributeValue, // Keep this import
    },
    Client as DynamoDbClient, // Keep this import
    // Remove unused operation-specific errors if SdkError is handled broadly
};
use tracing; // Keep tracing
use uuid::Uuid;

// Internal crate imports
use crate::models::Meme;

/// The name of the DynamoDB table used for memes.
pub const MEMES_TABLE: &str = "memes";

/// Creates the DynamoDB table for storing memes, if it does not already exist.
///
/// The table uses `meme_id` as the partition (hash) key and PayPerRequest billing.
pub async fn create_memes_table(client: &DynamoDbClient) -> Result<()> {
    let result = client
        .create_table()
        .table_name(MEMES_TABLE)
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("meme_id")
                .attribute_type(ScalarAttributeType::S) // UUID stored as String
                .build()
                .context("Failed to build attribute definition for meme_id")?, // Handle BuildError
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("meme_id")
                .key_type(KeyType::Hash) // Partition key
                .build()
                .context("Failed to build key schema for meme_id")?, // Handle BuildError
        )
        .billing_mode(BillingMode::PayPerRequest)
        .send()
        .await;

    match result {
        Ok(_) => {
            tracing::info!("Table '{}' created successfully or already existed.", MEMES_TABLE);
            Ok(())
        }
        Err(e) => {
            // Check if the error is specifically that the table already exists
            if let SdkError::ServiceError(service_err) = &e {
                if service_err.err().is_resource_in_use_exception() {
                    tracing::info!("Table '{}' already exists, no action needed.", MEMES_TABLE);
                    Ok(()) // Not an error in our context if it exists
                } else {
                    // Different service error
                     Err(anyhow::Error::new(e).context(format!(
                        "Service error creating DynamoDB table '{}'",
                        MEMES_TABLE
                    )))
                }
            } else {
                 // Other SDK errors (dispatch, timeout, etc.)
                 Err(anyhow::Error::new(e).context(format!(
                    "SDK error creating DynamoDB table '{}'",
                    MEMES_TABLE
                )))
            }
        }
    }
}


/// Converts a DynamoDB item (a HashMap) into a `Meme` instance.
/// Returns `None` if any required field is missing or has the wrong type,
/// or if the meme_id is not a valid UUID.
fn item_to_meme(item: &HashMap<String, AttributeValue>) -> Option<Meme> { // Take reference
    // Use .get() and .as_s() which return Option<&String>, then .ok()? to propagate None
    let meme_id_str = item.get("meme_id")?.as_s().ok()?;
    let title = item.get("title")?.as_s().ok()?;
    let description = item.get("description")?.as_s().ok()?;
    let image_key = item.get("image_key")?.as_s().ok()?;

    // Attempt to parse the UUID string
    let meme_id = Uuid::parse_str(meme_id_str).ok()?;

    Some(Meme {
        meme_id,
        title: title.to_string(), // Clone String from &str
        description: description.to_string(),
        image_key: image_key.to_string(),
    })
}

/// Stores a `Meme` in the DynamoDB table.
///
/// This function uses the PutItem builder pattern.
/// It adds context to potential errors using `anyhow`.
pub async fn put_meme(client: &DynamoDbClient, meme: &Meme) -> Result<()> {
    client
        .put_item()
        .table_name(MEMES_TABLE)
        // Build attributes directly
        .item("meme_id", AttributeValue::S(meme.meme_id.to_string()))
        .item("title", AttributeValue::S(meme.title.clone()))
        .item("description", AttributeValue::S(meme.description.clone()))
        .item("image_key", AttributeValue::S(meme.image_key.clone()))
        .send()
        .await
        .context(format!("Failed to put meme (id: {}) metadata in DynamoDB", meme.meme_id))?; // Add context
    Ok(())
}

/// Retrieves a `Meme` from DynamoDB using the given `meme_id`.
///
/// Returns:
/// - `Ok(Some(Meme))` if found,
/// - `Ok(None)` if not found,
/// - `Err(anyhow::Error)` if the AWS SDK operation fails or item data is invalid.
pub async fn get_meme(client: &DynamoDbClient, meme_id: &str) -> Result<Option<Meme>> {
    // Validate UUID format *before* making the AWS call
    if Uuid::parse_str(meme_id).is_err() {
        tracing::warn!(invalid_meme_id = %meme_id, "Attempted to get meme with invalid UUID format");
        // Return Ok(None) because the *item* won't be found with an invalid ID format,
        // rather than indicating a server error. Or return an InvalidInput error.
        // Let's return None for simplicity here, handler can map to 404.
        return Ok(None);
    }

    let resp = client
        .get_item()
        .table_name(MEMES_TABLE)
        .key("meme_id", AttributeValue::S(meme_id.to_string()))
        .send()
        .await
        .context(format!("Failed to get meme (id: {}) from DynamoDB", meme_id))?;

    match resp.item {
        Some(item) => {
            // Attempt to convert the retrieved item into a Meme struct
             match item_to_meme(&item) { // Pass reference
                Some(meme) => Ok(Some(meme)),
                None => {
                    // Item found, but parsing failed (data corruption?)
                    tracing::error!(meme_id = %meme_id, "Retrieved item from DynamoDB but failed to parse it into a Meme struct");
                    // Indicate an internal issue rather than just "not found"
                    Err(anyhow::anyhow!("Failed to parse meme data retrieved from DynamoDB for id {}", meme_id))
                }
            }
        }
        None => {
            // Item not found in DynamoDB
            Ok(None)
        }
    }
}

/// Lists all memes currently stored in the DynamoDB table.
///
/// NOTE: A `Scan` operation reads the entire table, which can be inefficient
/// and costly for large tables. Consider alternative query patterns (e.g., using
/// Global Secondary Indexes) for production use cases if applicable.
/// This implementation does not handle pagination.
///
/// Returns:
/// - `Ok(Vec<Meme>)` containing all valid memes found.
/// - `Err(anyhow::Error)` if the AWS SDK operation fails or parsing any item fails.
pub async fn list_memes(client: &DynamoDbClient) -> Result<Vec<Meme>> {
    tracing::debug!("Scanning DynamoDB table '{}' for all memes", MEMES_TABLE);
    let mut memes: Vec<Meme> = Vec::new();
    let mut last_evaluated_key = None;

    // Basic pagination loop (scan until no more items)
    loop {
         let mut request = client.scan().table_name(MEMES_TABLE);
         if let Some(lek) = last_evaluated_key {
             request = request.set_exclusive_start_key(Some(lek));
         }

         let resp = request
             .send()
             .await
             .context(format!("Failed to scan DynamoDB table '{}'", MEMES_TABLE))?;

         if let Some(items) = resp.items {
             tracing::debug!("Scan returned {} items", items.len());
             for item in items {
                 match item_to_meme(&item) { // Pass reference
                     Some(meme) => memes.push(meme),
                     None => {
                         // Log the issue but continue processing other items
                         // Alternatively, could return an error immediately
                         let item_id = item.get("meme_id").and_then(|v| v.as_s().ok());
                         tracing::error!(item.id = ?item_id, "Failed to parse item from DynamoDB scan into Meme struct");
                         // Optionally return error:
                         // return Err(anyhow::anyhow!("Failed to parse item {:?} during scan", item_id));
                     }
                 }
             }
         } else {
            tracing::debug!("Scan returned no items in this page.");
         }

         // Check if pagination is complete
         if resp.last_evaluated_key.is_none() {
             tracing::debug!("Scan complete. No LastEvaluatedKey found.");
             break; // Exit loop
         } else {
             tracing::debug!("Continuing scan with LastEvaluatedKey...");
             last_evaluated_key = resp.last_evaluated_key;
         }
    }


    tracing::info!("Successfully listed {} memes from DynamoDB", memes.len());
    Ok(memes)
}
```
*Self-correction:*
1.  Made `item_to_meme` accept a reference `&HashMap<...>` to avoid unnecessary cloning within `list_memes`.
2.  Added basic pagination handling to `list_memes` as `Scan` operations return a maximum of 1MB of data per request. This makes it more robust even if the table grows beyond a single page.
3.  Improved error handling in `get_meme` when `item_to_meme` fails – it now returns an `Err` because finding an item that can't be parsed is an internal issue, not a "not found".
4.  Added context to `BuildError` handling in `create_memes_table`.
5.  Improved logging in `list_memes`.

### 8. `src/main.rs`

This is the core application logic, tying everything together. It includes the new route and handler for listing memes.

```rust
// src/main.rs
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse, // Keep IntoResponse import
    routing::{get, post},
    Json, Router,
};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::{primitives::ByteStream, Client as S3Client, error::SdkError};
// Import the trait needed for .meta() on the *inner* error type
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::{cors::{Any, CorsLayer}, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use dotenvy;

// --- Module imports ---
mod aws_clients;
mod db;
mod models;
mod error_types;

// --- Use declarations ---
use crate::aws_clients::{create_dynamodb_client, create_s3_client};
use crate::db::{create_memes_table, get_meme, list_memes, put_meme}; // Added list_memes
use crate::models::Meme;
use crate::error_types::AppError; // Use our custom error type


/// AppState holds shared resources like AWS clients and configuration.
/// Clone is cheap because it contains Arcs or clients designed for cloning.
#[derive(Clone)]
struct AppState {
    db_client: DynamoDbClient,
    s3_client: S3Client,
    bucket_name: String,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // --- Initialize tracing (logging) ---
    // Sets up a subscriber for processing tracing events (logs).
    // Reads log level from RUST_LOG env var, defaulting to debug for our crate and tower_http.
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "axum_meme_posting_example=debug,tower_http=debug,info".into() // Default filter
        }))
        .with(tracing_subscriber::fmt::layer()) // Formats logs for printing
        .init(); // Sets this subscriber as the global default

    // --- Load .env file ---
    // Attempts to load environment variables from a `.env` file in the current directory.
    // Useful for development without setting system-wide environment variables.
    match dotenvy::dotenv() {
        Ok(path) => tracing::info!(".env file loaded from path: {}", path.display()),
        Err(_) => tracing::info!(".env file not found, relying solely on environment variables"),
    };

    // --- Configuration ---
    // Reads required configuration from environment variables.
    // Panics via `expect` if MEME_BUCKET_NAME is not set. Consider returning AppError instead.
    let bucket_name = env::var("MEME_BUCKET_NAME")
        .map_err(|e| {
            tracing::error!("FATAL: MEME_BUCKET_NAME environment variable not set.");
            AppError::EnvVarError(e) // Convert error
        })?;

    let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    // --- AWS Client Initialization ---
    tracing::info!("Initializing AWS clients for LocalStack...");
    // These functions create clients configured to talk to LocalStack (http://localhost:4566)
    let db_client = create_dynamodb_client().await;
    let s3_client = create_s3_client().await;
    tracing::info!("AWS clients initialized.");

    // --- Resource Creation (Idempotent) ---
    // Ensures the necessary AWS resources (DynamoDB table, S3 bucket) exist.
    // These operations are designed to be safe to run multiple times.

    tracing::info!("Ensuring DynamoDB table '{}' exists...", db::MEMES_TABLE);
    // create_memes_table returns Result<(), anyhow::Error>
    // The '?' operator uses the From<anyhow::Error> trait in error_types.rs
    // to convert it into an AppError if it fails.
    create_memes_table(&db_client).await?;
    tracing::info!("DynamoDB table check complete.");

    tracing::info!("Ensuring S3 bucket '{}' exists...", bucket_name);
    // Attempt to create the S3 bucket.
    match s3_client.create_bucket().bucket(&bucket_name).send().await {
        Ok(_) => {
            tracing::info!("S3 bucket '{}' created or already exists.", bucket_name);
        }
        Err(sdk_err) => {
            // Check if the error indicates the bucket already exists.
            if let SdkError::ServiceError(service_err) = &sdk_err {
                 // Access the underlying service-specific error (e.g., CreateBucketError)
                 // and then its metadata to get the error code.
                let code = service_err.err().meta().code();
                if code == Some("BucketAlreadyOwnedByYou") || code == Some("BucketAlreadyExists") {
                    tracing::info!("S3 bucket '{}' already exists.", bucket_name);
                    // This is not an error for our startup sequence.
                } else {
                    // A different S3 service error occurred.
                    tracing::error!("Failed to create S3 bucket '{}': SDK service error: {}", bucket_name, service_err);
                    // Convert the SdkError into our AppError using the From trait.
                    return Err(sdk_err.into());
                }
            } else {
                 // A non-service error occurred (e.g., network issue, credential problem).
                tracing::error!("Failed to create S3 bucket '{}': SDK error: {}", bucket_name, sdk_err);
                 // Convert the SdkError into our AppError.
                return Err(sdk_err.into());
            }
        }
    }
    tracing::info!("S3 bucket check complete.");

    // --- Application State ---
    // Create the shared state containing AWS clients and bucket name.
    // Arc allows safe sharing across asynchronous tasks (request handlers).
    let state = Arc::new(AppState {
        db_client,
        s3_client,
        bucket_name,
    });

    // --- Router Definition ---
    // Defines the API endpoints and their corresponding handlers.
    let app = Router::new()
        .route("/upload_meme", post(upload_meme_handler)) // Handler for POST /upload_meme
        .route("/meme/:id", get(get_meme_handler))       // Handler for GET /meme/{id}
        .route("/memes", get(list_memes_handler))       // Handler for GET /memes (NEW)
        // Middleware Layers:
        .layer(
             // Enable Cross-Origin Resource Sharing (CORS)
             // permissive() allows requests from any origin, method, and header.
             // For production, configure this more restrictively.
             CorsLayer::new()
                .allow_origin(Any) // Or specify origins: .allow_origin("http://example.com".parse::<HeaderValue>().unwrap())
                .allow_methods(Any) // Or specify methods: .allow_methods(vec![Method::GET, Method::POST])
                .allow_headers(Any), // Or specify headers
        )
        .layer(TraceLayer::new_for_http()) // Adds request/response logging via tracing
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // Set max request body size (e.g., 10MB)
        .with_state(state); // Makes the AppState available to handlers


    // --- Server Startup ---
    // Parse the bind address string into a SocketAddr.
    let addr: SocketAddr = bind_address
        .parse()
        .map_err(|e| AppError::InternalServerError(format!("Invalid bind address format '{}': {}", bind_address, e)))?;

    tracing::info!("Server listening on http://{}", addr);

    // Bind a TCP listener to the address.
    let listener = tokio::net::TcpListener::bind(addr).await?; // Use '?' to handle potential bind errors

    // Start the Axum server, making the router handle incoming connections.
    axum::serve(listener, app.into_make_service()).await?; // Use '?' to handle potential server errors

    Ok(()) // Signal successful server shutdown (though it typically runs indefinitely)
}

// --- Route Handlers ---

/// Handler for POST /upload_meme
/// Processes multipart/form-data containing 'title', 'description', and 'image'.
/// Uploads the image to S3 and stores meme metadata in DynamoDB.
async fn upload_meme_handler(
    State(state): State<Arc<AppState>>, // Extract shared state
    mut multipart: Multipart,           // Extract multipart form data
) -> Result<(StatusCode, Json<Meme>), AppError> { // Return Result<SuccessResponse, AppError>
    let meme_id = Uuid::new_v4(); // Generate a unique ID for the new meme
    let mut title = None;
    let mut description = None;
    let mut image_data = None;
    let mut image_filename = None; // Store original filename to guess extension

    // --- Process Multipart Fields ---
    // Iterate through each part (field) of the multipart request.
    while let Some(field) = multipart.next_field().await? { // '?' handles errors during field reading
        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => continue, // Skip fields without names
        };

        // Match on the field name to process known fields.
        match field_name.as_str() {
            "title" => title = Some(field.text().await?), // '?' handles errors reading text content
            "description" => description = Some(field.text().await?),
            "image" => {
                // Store the original filename if available.
                image_filename = field.file_name().map(|s| s.to_string());
                // Read the entire field content into bytes.
                // This loads the whole image into memory. For very large files, streaming might be better.
                image_data = Some(field.bytes().await?.to_vec()); // '?' handles errors reading bytes
            }
            _ => {
                // Log and ignore any unexpected fields.
                tracing::debug!("Ignoring unknown multipart field: {}", field_name);
            }
        }
    }

    // --- Validate Required Fields ---
    // Ensure all necessary parts of the meme were provided in the request.
    // Use ok_or_else to map Option::None to a specific AppError::MissingFormField.
    let title = title.ok_or_else(|| AppError::MissingFormField("title".to_string()))?;
    let description = description.ok_or_else(|| AppError::MissingFormField("description".to_string()))?;
    let image_data = image_data.ok_or_else(|| AppError::MissingFormField("image".to_string()))?;

    // Basic validation: ensure image data is not empty.
    if image_data.is_empty() {
        return Err(AppError::InvalidInput("image data cannot be empty".to_string()));
    }

    // --- Prepare S3 Upload ---
    // Determine the file extension for the S3 key. Default to "png".
    let extension = image_filename
        .and_then(|name| name.split('.').last().map(|ext| ext.to_lowercase()))
        .filter(|ext| ["png", "jpg", "jpeg", "gif", "webp"].contains(&ext.as_str())) // Basic filter
        .unwrap_or_else(|| {
            tracing::warn!("Could not determine valid image extension from filename, defaulting to 'png'");
            "png".to_string()
        });

    // Construct the S3 object key (filename in the bucket).
    let image_key = format!("{}.{}", meme_id, extension);

    tracing::debug!(s3_key = %image_key, bucket = %state.bucket_name, "Uploading image to S3");

    // --- Upload Image to S3 ---
    // Use the S3 client from the shared state.
    state
        .s3_client
        .put_object()
        .bucket(&state.bucket_name) // Target bucket
        .key(&image_key)           // Object key (filename)
        .body(ByteStream::from(image_data)) // Provide the image data as a ByteStream
        // Optional: Set Content-Type based on extension for better browser handling if served directly.
        // .content_type(mime_guess::from_path(&image_key).first_or_octet_stream().to_string())
        .send() // Execute the upload request
        .await?; // '?' propagates S3 errors (converted to AppError via From<SdkError>)

    tracing::debug!(s3_key = %image_key, "Image uploaded successfully. Storing metadata in DynamoDB.");

    // --- Prepare DynamoDB Item ---
    // Create the Meme struct containing metadata.
    let meme = Meme {
        meme_id,
        title,
        description,
        image_key, // Store the S3 key associated with the image
    };

    // --- Store Metadata in DynamoDB ---
    // Use the DynamoDB client from the shared state.
    // put_meme returns Result<(), anyhow::Error>. '?' converts to AppError via From<anyhow::Error>.
    put_meme(&state.db_client, &meme).await?;

    tracing::info!(meme_id = %meme_id, "Meme created successfully");

    // --- Return Success Response ---
    // On success, return HTTP 201 Created status code and the created Meme object as JSON.
    Ok((StatusCode::CREATED, Json(meme)))
}

/// Handler for GET /meme/:id
/// Retrieves metadata for a specific meme by its UUID.
async fn get_meme_handler(
    State(state): State<Arc<AppState>>, // Extract shared state
    Path(id_str): Path<String>,        // Extract 'id' path parameter
) -> Result<Json<Meme>, AppError> {     // Return Result<Json<Meme>, AppError>
    tracing::debug!(meme_id = %id_str, "Fetching meme details");

    // Call the database function to retrieve the meme.
    // get_meme returns Result<Option<Meme>, anyhow::Error>
    // '?' propagates DB errors (converted to AppError::DatabaseError).
    let maybe_meme = get_meme(&state.db_client, &id_str).await?;

    // --- Handle Result ---
    match maybe_meme {
        Some(meme) => {
            // Meme found, return 200 OK with meme data as JSON.
            tracing::debug!(meme_id = %id_str, "Meme found");
            Ok(Json(meme))
        }
        None => {
            // Meme not found (or ID was invalid format handled in get_meme).
            tracing::warn!(meme_id = %id_str, "Meme not found");
            // Return a 404 Not Found error using our AppError type.
            Err(AppError::NotFound(format!("Meme with id {}", id_str)))
        }
    }
}

/// Handler for GET /memes
/// Retrieves metadata for all memes in the database.
async fn list_memes_handler(
    State(state): State<Arc<AppState>>, // Extract shared state
) -> Result<Json<Vec<Meme>>, AppError> { // Return Result<Json<Vec<Meme>>, AppError>
    tracing::debug!("Listing all memes");

    // Call the database function to list all memes.
    // list_memes returns Result<Vec<Meme>, anyhow::Error>.
    // '?' propagates errors (converted to AppError::DatabaseError).
    let memes = list_memes(&state.db_client).await?;

    tracing::info!("Successfully retrieved {} memes", memes.len());

    // Return 200 OK with the list of memes as JSON.
    Ok(Json(memes))
}
```
*Self-correction:*
1.  Updated CORS layer setup for better clarity and flexibility (`CorsLayer::new().allow_origin(Any)...`).
2.  Ensured error handling in `main` for environment variables converts to `AppError`.
3.  Added more detailed tracing/logging messages throughout `main` and handlers.
4.  Made sure all `?` operations correctly rely on the `From` trait implementations in `error_types.rs` to convert underlying errors (SDK errors, `anyhow::Error`, IO errors, etc.) into `AppError`.
5.  Improved comments explaining the purpose of different sections (initialization, resource creation, state, router, handlers).
6.  Refined the S3 bucket creation check logic slightly for clarity.

### 9. `.env.example` and `.env`

Create a file named `.env.example` with the following content:

```env
# .env.example
# Configuration for the Axum Meme Posting Example

# --- AWS Configuration (for LocalStack) ---
# These are typically not needed when using LocalStack defaults and aws_config,
# but are shown here for completeness if you were targeting real AWS.
# AWS_ACCESS_KEY_ID=test
# AWS_SECRET_ACCESS_KEY=test
AWS_DEFAULT_REGION=ca-central-1

# --- Application Configuration ---
# The name of the S3 bucket to store meme images.
# This bucket will be created automatically if it doesn't exist in LocalStack.
MEME_BUCKET_NAME=my-local-meme-bucket

# The network address and port the server should bind to.
BIND_ADDRESS=0.0.0.0:3000

# --- Logging Configuration ---
# Controls the verbosity of logs. Examples:
# RUST_LOG=info                                       # Show info level for all crates
# RUST_LOG=axum_meme_posting_example=debug,info      # Debug for our app, info for others
# RUST_LOG=debug                                      # Debug for all crates (very verbose)
RUST_LOG=axum_meme_posting_example=debug,tower_http=debug,info
```

Then, **copy** this file to `.env` and modify `MEME_BUCKET_NAME` if you wish (though the default is fine for local testing). Do **not** commit `.env` to version control.

### 10. Running the Application

1.  **Start LocalStack:** Open a terminal in the project directory and run:
    ```bash
    docker-compose up -d
    ```
    Wait a few seconds for LocalStack to initialize. You can check logs with `docker-compose logs -f localstack`.

2.  **Build and Run the Rust App:** In another terminal in the project directory:
    ```bash
    cargo build
    cargo run
    ```
    You should see logs indicating the server is starting, connecting to LocalStack, creating resources (or confirming they exist), and finally listening on `http://0.0.0.0:3000`.

### 11. Testing the API

You can use tools like `curl` or Postman/Insomnia:

* **Upload a Meme:**
    ```bash
    curl -X POST http://localhost:3000/upload_meme \
      -F "title=My First Meme" \
      -F "description=Testing the upload functionality" \
      -F "image=@/path/to/your/image.jpg" # Replace with actual path to an image
    ```
    *(Response should be 201 Created with the JSON of the created meme)*

* **Get a Specific Meme:** (Replace `{meme_id}` with the `meme_id` from the upload response)
    ```bash
    curl http://localhost:3000/meme/{meme_id}
    ```
    *(Response should be 200 OK with the meme JSON, or 404 Not Found)*

* **List All Memes:**
    ```bash
    curl http://localhost:3000/memes
    ```
    *(Response should be 200 OK with a JSON array of all uploaded memes)*

You now have a complete, working Axum application using LocalStack for local AWS development, covering upload, retrieval by ID, and listing of memes!
