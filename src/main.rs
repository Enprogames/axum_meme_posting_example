// --- Crates and Modules ---
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State}, // Core Axum extractors
    http::StatusCode,                                    // HTTP status codes
    routing::{get, post},                                // Routing functions
    Json, Router,                                        // JSON handling and Router
};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::{
    primitives::ByteStream, // For streaming request bodies (like image uploads)
    Client as S3Client,
    error::SdkError,                                     // General AWS SDK Error type
    types::{BucketLocationConstraint, CreateBucketConfiguration}, // Types for S3 bucket creation
};
use std::{env, net::SocketAddr, sync::Arc};              // Standard library items
use tower_http::{
    cors::{Any, CorsLayer}, // CORS middleware
    trace::TraceLayer,     // Request/response tracing middleware
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter}; // Logging/Tracing setup
use uuid::Uuid;             // For generating unique IDs
use dotenvy;                // For loading .env files

// --- Project Modules ---
mod aws_clients;
mod db;
mod models;
mod error_types;

// --- Project Imports ---
use crate::aws_clients::{create_dynamodb_client, create_s3_client}; // Functions to create AWS clients
use crate::db::{create_memes_table, get_meme, list_memes, put_meme}; // Database interaction functions
use crate::models::Meme;                                             // Data structure for a Meme
use crate::error_types::AppError;                                    // Custom application error type

//-----------------------------------------------------------------------------
// Application State
//-----------------------------------------------------------------------------

/// Holds shared resources like AWS clients and configuration, accessible by handlers.
/// `Arc` allows safe, shared, read-only access across multiple threads/tasks.
/// `Clone` is efficient as it only increments the Arc's reference count.
#[derive(Clone)]
struct AppState {
    db_client: DynamoDbClient,
    s3_client: S3Client,
    bucket_name: String,
}

//-----------------------------------------------------------------------------
// Main Entry Point
//-----------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), AppError> { // Return AppError on fatal startup issues

    // --- Initialize Tracing (Logging) ---
    // Sets up logging infrastructure using the `tracing` crate.
    // Reads log level filters from the RUST_LOG environment variable.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // Default filter if RUST_LOG is not set
            "axum_meme_posting_example=debug,tower_http=debug,info".into()
        }))
        .with(tracing_subscriber::fmt::layer()) // Formats logs for console output
        .init(); // Sets this configuration as the global default

    // --- Load .env File ---
    // Loads environment variables from a `.env` file if present (useful for development).
    match dotenvy::dotenv() {
        Ok(path) => tracing::info!(".env file loaded from path: {}", path.display()),
        Err(_) => tracing::info!(".env file not found, relying solely on environment variables"),
    };

    // --- Load Configuration ---
    // Retrieves required configuration values from environment variables.
    let bucket_name = env::var("MEME_BUCKET_NAME")
        .map_err(|e| {
            tracing::error!("FATAL: MEME_BUCKET_NAME environment variable not set.");
            AppError::EnvVarError(e) // Convert VarError to AppError
        })?;

    let bind_address = env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string()); // Default bind address

    // --- AWS Client Initialization ---
    // Creates AWS service clients configured to interact with LocalStack.
    tracing::info!("Initializing AWS clients for LocalStack...");
    let db_client = create_dynamodb_client().await;
    let s3_client = create_s3_client().await;
    tracing::info!("AWS clients initialized.");

    // --- Create Application State ---
    // Bundle shared resources into an Arc<AppState> for handlers.
    // Must be created *before* resource checks that need client config (like S3 region).
    let state = Arc::new(AppState {
        db_client: db_client.clone(), // Clone clients into state
        s3_client: s3_client.clone(),
        bucket_name: bucket_name.clone(),
    });

    // --- Ensure AWS Resources Exist (Idempotent) ---
    // These calls attempt to create resources if they don't exist, safely handling cases
    // where they already do. Essential for local development setup.

    // Ensure DynamoDB table exists
    tracing::info!("Ensuring DynamoDB table '{}' exists...", db::MEMES_TABLE);
    create_memes_table(&db_client).await?; // Uses `?` to propagate AppError on failure
    tracing::info!("DynamoDB table check complete.");

    // Ensure S3 bucket exists
    tracing::info!("Ensuring S3 bucket '{}' exists...", state.bucket_name);
    ensure_s3_bucket_exists(&state).await?; // Use helper function for clarity
    tracing::info!("S3 bucket check complete.");

    // --- Define API Routes (Router) ---
    // Configures the Axum router, mapping HTTP paths and methods to handler functions.
    let app = Router::new()
        .route("/upload_meme", post(upload_meme_handler)) // POST /upload_meme -> upload_meme_handler
        .route("/meme/{id}", get(get_meme_handler))       // GET /meme/{uuid} -> get_meme_handler
        .route("/memes", get(list_memes_handler))       // GET /memes -> list_memes_handler
        // Apply Middleware Layers:
        .layer(
             // Permissive CORS configuration (allow all origins/methods/headers).
             // TODO: Restrict this for production environments.
             CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http()) // Log request/response details
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // Set max request body size (e.g., 10MB for uploads)
        .with_state(state); // Make the AppState available to all handlers

    // --- Start HTTP Server ---
    let addr: SocketAddr = bind_address
        .parse()
        .map_err(|e| AppError::InternalServerError(format!("Invalid bind address format '{}': {}", bind_address, e)))?;

    tracing::info!("Server listening on http://{}", addr);

    // Bind the TCP listener
    let listener = tokio::net::TcpListener::bind(addr).await?; // Propagate bind errors

    // Run the Axum server
    axum::serve(listener, app.into_make_service()).await?; // Propagate server errors

    Ok(()) // Indicate successful (although typically indefinite) server run
}

//-----------------------------------------------------------------------------
// Helper Functions
//-----------------------------------------------------------------------------

/// Checks if the S3 bucket exists and attempts to create it if not.
/// Handles region constraints required by S3.
async fn ensure_s3_bucket_exists(state: &Arc<AppState>) -> Result<(), AppError> {
    // Get region from the S3 client's resolved configuration
    let client_region = state.s3_client.config().region()
        .ok_or_else(|| AppError::InternalServerError("S3 client region not configured".to_string()))?;
    let region_str = client_region.as_ref(); // Region as &str

    tracing::debug!("S3 client configured for region: {}", region_str);

    // Create bucket configuration with LocationConstraint *unless* the region is us-east-1.
    let bucket_config = if region_str != "us-east-1" {
        Some(
            CreateBucketConfiguration::builder()
                .location_constraint(BucketLocationConstraint::from(region_str))
                .build(),
        )
    } else {
        None // No constraint needed for us-east-1
    };

    // Build the CreateBucket request, adding the configuration if necessary
    let mut create_bucket_req_builder = state.s3_client.create_bucket().bucket(&state.bucket_name);
    if let Some(config) = bucket_config {
        create_bucket_req_builder = create_bucket_req_builder.create_bucket_configuration(config);
    }

    // Send the request and handle potential errors
    match create_bucket_req_builder.send().await {
        Ok(_) => {
            tracing::info!("S3 bucket '{}' created or already exists.", state.bucket_name);
            Ok(())
        }
        Err(sdk_err) => {
            // Check if it's a ServiceError (API error) vs other SDK issues
            if let SdkError::ServiceError(service_err) = &sdk_err {
                // Get the specific error code (e.g., "BucketAlreadyOwnedByYou")
                let code = service_err.err().meta().code();
                if code == Some("BucketAlreadyOwnedByYou") || code == Some("BucketAlreadyExists") {
                    tracing::info!("S3 bucket '{}' already exists.", state.bucket_name);
                    Ok(()) // Bucket already exists, which is fine for our setup
                } else {
                    // A different S3 service error occurred. Log and propagate.
                    // Use debug formatting {:?} for service_err as it doesn't implement Display
                    tracing::error!("Failed to create S3 bucket '{}': SDK service error: {:?}", state.bucket_name, service_err);
                    Err(AppError::from(sdk_err)) // Convert SdkError to AppError
                }
            } else {
                 // A non-service error (network, credentials, etc.). Log and propagate.
                 // SdkError itself implements Display, so {} is okay here.
                tracing::error!("Failed to create S3 bucket '{}': SDK error: {}", state.bucket_name, sdk_err);
                Err(AppError::from(sdk_err)) // Convert SdkError to AppError
            }
        }
    }
}


//-----------------------------------------------------------------------------
// Route Handlers
//-----------------------------------------------------------------------------

/// Handler for POST /upload_meme
/// Processes multipart form data (title, description, image), uploads image to S3,
/// stores metadata in DynamoDB.
async fn upload_meme_handler(
    State(state): State<Arc<AppState>>, // Access shared state
    mut multipart: Multipart,           // Axum extractor for multipart/form-data
) -> Result<(StatusCode, Json<Meme>), AppError> { // Return 201 Created + Meme on success
    let meme_id = Uuid::new_v4(); // Generate unique ID
    let mut title = None;
    let mut description = None;
    let mut image_data: Option<Vec<u8>> = None;
    let mut image_filename = None;

    // Process each part of the multipart request
    while let Some(field) = multipart.next_field().await? { // Propagate multipart processing errors
        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => continue, // Skip unnamed fields
        };

        match field_name.as_str() {
            "title" => title = Some(field.text().await?), // Read text field, propagate errors
            "description" => description = Some(field.text().await?),
            "image" => {
                image_filename = field.file_name().map(|s| s.to_string());
                // Read binary data into memory. Consider streaming for very large files.
                image_data = Some(field.bytes().await?.to_vec());
            }
            _ => tracing::debug!("Ignoring unknown multipart field: {}", field_name),
        }
    }

    // Validate required fields were received
    let title = title.ok_or_else(|| AppError::MissingFormField("title".to_string()))?;
    let description = description.ok_or_else(|| AppError::MissingFormField("description".to_string()))?;
    let image_data = image_data.ok_or_else(|| AppError::MissingFormField("image".to_string()))?;

    // Basic validation
    if image_data.is_empty() {
        return Err(AppError::InvalidInput("image data cannot be empty".to_string()));
    }

    // Determine S3 key extension (defaulting to png)
    let extension = image_filename
        .and_then(|name| name.split('.').last().map(|ext| ext.to_lowercase()))
        .filter(|ext| ["png", "jpg", "jpeg", "gif", "webp"].contains(&ext.as_str())) // Allow common types
        .unwrap_or_else(|| {
            tracing::warn!("Could not determine valid image extension from filename, defaulting to 'png'");
            "png".to_string()
        });
    let image_key = format!("{}.{}", meme_id, extension); // S3 object key

    // Upload image to S3
    tracing::debug!(s3_key = %image_key, bucket = %state.bucket_name, "Uploading image to S3");
    state
        .s3_client
        .put_object()
        .bucket(&state.bucket_name)
        .key(&image_key)
        .body(ByteStream::from(image_data)) // Create ByteStream from Vec<u8>
        .send()
        .await?; // Propagate S3 errors (converted to AppError)

    // Store metadata in DynamoDB
    tracing::debug!(s3_key = %image_key, "Image uploaded successfully. Storing metadata in DynamoDB.");
    let meme = Meme {
        meme_id,
        title,
        description,
        image_key, // Reference to the S3 object
    };
    put_meme(&state.db_client, &meme).await?; // Propagate DynamoDB errors

    tracing::info!(meme_id = %meme_id, "Meme created successfully");

    // Return 201 Created status and the meme data
    Ok((StatusCode::CREATED, Json(meme)))
}

/// Handler for GET /meme/:id
/// Retrieves metadata for a single meme by its UUID.
async fn get_meme_handler(
    State(state): State<Arc<AppState>>, // Access shared state
    Path(id_str): Path<String>,        // Extract UUID string from path
) -> Result<Json<Meme>, AppError> {    // Return 200 OK + Meme on success, or AppError
    tracing::debug!(meme_id = %id_str, "Fetching meme details");

    // `get_meme` handles UUID parsing and DB interaction
    let maybe_meme = get_meme(&state.db_client, &id_str).await?; // Propagate DB errors

    // Check if the meme was found
    match maybe_meme {
        Some(meme) => {
            tracing::debug!(meme_id = %id_str, "Meme found");
            Ok(Json(meme)) // Return 200 OK with JSON body
        }
        None => {
            tracing::warn!(meme_id = %id_str, "Meme not found");
            Err(AppError::NotFound(format!("Meme with id {}", id_str))) // Return 404 Not Found
        }
    }
}

/// Handler for GET /memes
/// Retrieves metadata for all memes in the database.
async fn list_memes_handler(
    State(state): State<Arc<AppState>>, // Access shared state
) -> Result<Json<Vec<Meme>>, AppError> { // Return 200 OK + Vec<Meme> on success
    tracing::debug!("Listing all memes");

    // `list_memes` handles scanning the DynamoDB table
    let memes = list_memes(&state.db_client).await?; // Propagate DB errors

    tracing::info!("Successfully retrieved {} memes", memes.len());

    // Return 200 OK with JSON array of memes
    Ok(Json(memes))
}
