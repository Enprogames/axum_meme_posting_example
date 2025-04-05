use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::StatusCode,
    // Note: Don't need response::IntoResponse/Response here directly
    routing::{get, post},
    Json, Router,
};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::{primitives::ByteStream, Client as S3Client};
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
// Remove tracing::log::error if using tracing directly
// use tracing::log::error;
use dotenvy; // <-- Add this if using .env files

// --- Assume these modules exist and contain the necessary functions ---
mod aws_clients;
mod db;
mod models;
mod error_types; // Your error module

use crate::aws_clients::{create_dynamodb_client, create_s3_client}; // Use crate:: prefix
use crate::db::{create_memes_table, get_meme, put_meme}; // Use crate:: prefix
use crate::models::Meme; // Use crate:: prefix
use crate::error_types::AppError; // Use crate:: prefix to import AppError
// --- End assumed modules ---

/// AppState holds shared resources for the web server.
#[derive(Clone)]
struct AppState {
    db_client: DynamoDbClient,
    s3_client: S3Client,
    bucket_name: String,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize tracing (logging)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "axum_meme_posting_example=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load .env file if present (optional, good for development)
    match dotenvy::dotenv() {
        Ok(path) => tracing::info!(".env file loaded from path: {}", path.display()),
        Err(_) => tracing::info!(".env file not found, relying on environment variables"), // Don't error if not found
    };


    // --- Configuration ---
    let bucket_name = env::var("MEME_BUCKET_NAME")?; // Use ? directly thanks to From<env::VarError>
    let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    // --- AWS Client Initialization ---
    // Assuming create_..._client returns Result<Client, anyhow::Error> or compatible
    tracing::info!("Initializing AWS DynamoDB client...");
    let db_client_result = create_dynamodb_client().await; // This should yield Result<Client, E>
    let db_client = db_client_result.map_err(|e| {
        // Log the original error before mapping
        tracing::error!("Failed to create DynamoDB client: {}", e);
        AppError::InternalServerError(format!("Failed to create DynamoDB client: {}", e))
    })?; // Apply map_err to the Result, then use ?

    tracing::info!("Initializing AWS S3 client...");
    let s3_client_result = create_s3_client().await; // This should yield Result<Client, E>
    let s3_client = s3_client_result.map_err(|e| {
        // Log the original error before mapping
        tracing::error!("Failed to create S3 client: {}", e);
        AppError::InternalServerError(format!("Failed to create S3 client: {}", e))
    })?; // Apply map_err to the Result, then use ?


    // --- Resource Creation (Consider moving outside app startup) ---
    // NOTE: Creating resources here isn't ideal for production.
    // Use IaC (Terraform, CDK, etc.) or manual setup.
    // This code might fail if resources exist or permissions are wrong.
    tracing::info!("Attempting to ensure DynamoDB table exists...");
    // Use map_err to convert potential anyhow::Error from db module into AppError
    create_memes_table(&db_client).await.map_err(|e| {
        tracing::warn!("Failed to create/verify DynamoDB table (may already exist or permissions issue): {}", e);
        // Decide if this is a fatal error for startup. Here we wrap it.
        // You might want to inspect 'e' further if db module returns specific errors.
        AppError::DatabaseError(e)
    })?; // Propagate as AppError if it's not Ok(())

    tracing::info!("DynamoDB table check complete.");


    tracing::info!("Attempting to ensure S3 bucket '{}' exists...", bucket_name);
    // The create_bucket call might fail if the bucket exists.
    // A better approach is to assume it exists or use head_bucket to check.
    // The generic From<SdkError<E>> will convert the AWS error to AppError::AwsSdkError
    match s3_client.create_bucket().bucket(&bucket_name).send().await {
        Ok(_) => {
            tracing::info!("S3 bucket '{}' created or already exists.", bucket_name);
        }
        Err(sdk_err) => {
            // The generic From<SdkError<E>> handles the conversion for ?.
            // However, here we want to inspect the error potentially.
            // For now, we just log it. If you need to ignore "already exists",
            // you would inspect `sdk_err.into_service_error()` here.
            tracing::warn!(
                "Create S3 bucket call finished for '{}'. Error (if any): {}",
                bucket_name,
                sdk_err
            );
             // Decide if this is fatal. For now, we let it proceed.
             // If it *was* fatal: return Err(sdk_err.into());
        }
    }


    // --- Application State ---
    let state = Arc::new(AppState {
        db_client,
        s3_client,
        bucket_name,
    });

    // --- Router Definition ---
    let app = Router::new()
        .route("/upload_meme", post(upload_meme_handler))
        .route("/meme/:id", get(get_meme_handler))
        // Use permissive CORS for simplicity; configure stricter rules for production
        .layer(CorsLayer::permissive())
        // Add tracing layer for logging requests
        .layer(TraceLayer::new_for_http())
        // Limit request body size (e.g., 10MB for uploads)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB limit
        .with_state(state);

    // --- Server Startup ---
    let addr: SocketAddr = bind_address
        .parse()
        .map_err(|e| AppError::InternalServerError(format!("Invalid bind address format '{}': {}", bind_address, e)))?;

    tracing::info!("Server listening on http://{}", addr); // Added http:// for clarity

    let listener = tokio::net::TcpListener::bind(addr).await?; // Use ? with From<std::io::Error>
    axum::serve(listener, app.into_make_service()) // Use .into_make_service() for axum 0.7/0.8 compatibility if needed, or just `app` for newer Tower
        .await?; // Use ? with From<std::io::Error> based on hyper 1.x

    Ok(())
}

// --- Handlers (upload_meme_handler, get_meme_handler) remain the same as the previous good version ---
// Make sure they return Result<..., AppError> and use `?` operator

/// Handler for uploading a meme.
/// (Code remains the same as the previous corrected version)
async fn upload_meme_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Meme>), AppError> { // Return Result<SuccessResponse, AppError>
    let meme_id = Uuid::new_v4();
    let mut title = None;
    let mut description = None;
    let mut image_data = None;
    let mut image_filename = None; // Optional: capture original filename

    // Process multipart fields more robustly
    while let Some(field) = multipart.next_field().await? { // Use ? for multipart errors
        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => continue, // Skip unnamed fields
        };

        match field_name.as_str() {
            "title" => title = Some(field.text().await?), // Use ? for text extraction errors
            "description" => description = Some(field.text().await?),
            "image" => {
                image_filename = field.file_name().map(|s| s.to_string());
                image_data = Some(field.bytes().await?.to_vec()); // Use ? for byte extraction errors
            }
            _ => {
                tracing::debug!("Ignoring unknown multipart field: {}", field_name);
            }
        }
    }

    // Validate required fields
    let title = title.ok_or_else(|| AppError::MissingFormField("title".to_string()))?;
    let description =
        description.ok_or_else(|| AppError::MissingFormField("description".to_string()))?;
    let image_data =
        image_data.ok_or_else(|| AppError::MissingFormField("image".to_string()))?;

    // Basic validation (example: check image data is not empty)
    if image_data.is_empty() {
        return Err(AppError::MissingFormField(
            "image data cannot be empty".to_string(),
        ));
    }

    // Determine image key (using extension from filename if possible, else default)
    let extension = image_filename
        .and_then(|name| name.split('.').last().map(|ext| ext.to_lowercase()))
        .filter(|ext| ["png", "jpg", "jpeg", "gif"].contains(&ext.as_str())) // Basic filter
        .unwrap_or_else(|| "png".to_string()); // Default to png if no/invalid extension

    let image_key = format!("{}.{}", meme_id, extension);

    tracing::debug!(s3_key = %image_key, "Uploading image to S3"); // Use structured logging

    // Upload image to S3
    state
        .s3_client
        .put_object()
        .bucket(&state.bucket_name)
        .key(&image_key)
        .body(ByteStream::from(image_data)) // Use ByteStream
        // .content_type(...) // Optional: set based on detected extension
        .send()
        .await?; // Use ? to propagate S3 errors (converted via From trait)

    tracing::debug!(s3_key = %image_key, "Image uploaded successfully. Storing metadata in DynamoDB.");

    // Build Meme metadata
    let meme = Meme {
        meme_id, // Assuming Meme struct uses Uuid directly
        title,
        description,
        image_key, // Store the S3 key
    };

    // Store meme metadata in DynamoDB
    // Assuming put_meme now returns Result<(), anyhow::Error> or similar
    put_meme(&state.db_client, &meme)
        .await?; // Use ? to propagate DB errors (converted via From trait)

    tracing::info!(meme_id = %meme_id, "Meme created successfully"); // Use structured logging

    // Return 201 Created status code on success
    Ok((StatusCode::CREATED, Json(meme)))
}


/// Handler for retrieving meme metadata by its ID.
/// (Code remains the same as the previous corrected version)
async fn get_meme_handler(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
) -> Result<Json<Meme>, AppError> { // Return Result<Json<Meme>, AppError>
    // Optional: Validate if id_str is a valid UUID format before querying
     if Uuid::parse_str(&id_str).is_err() {
         tracing::warn!(invalid_id = %id_str, "Received request with invalid UUID format");
         // You might want a specific AppError::BadRequest variant here
         return Err(AppError::InternalServerError("Invalid ID format".to_string()));
     };

    tracing::debug!(meme_id = %id_str, "Fetching meme"); // Use structured logging

    // Assume get_meme returns Result<Option<Meme>, anyhow::Error>
    let maybe_meme = get_meme(&state.db_client, &id_str)
        .await?; // Propagate DB errors (already converted to AppError::DatabaseError)

    match maybe_meme {
        Some(meme) => {
            tracing::debug!(meme_id = %id_str, "Meme found");
            Ok(Json(meme)) // Return 200 OK with meme data
        }
        None => {
            tracing::warn!(meme_id = %id_str, "Meme not found");
            Err(AppError::NotFound(format!("Meme with id {}", id_str))) // Return 404 Not Found
        }
    }
}