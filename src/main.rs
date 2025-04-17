use crate::{
    config::Config,
    domain::{FileStorage, MemeRepository},
    errors::AppError,
    repositories::DynamoDbMemeRepository,
    routes::create_router,
    startup::init_resources,
    storage::S3FileStorage,
};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::Client as S3Client;
use tokio::signal;
use tracing::info;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// --- Modules ---
mod aws_clients;
mod config;
mod domain;
mod errors;
mod handlers;
mod models;
mod repositories;
mod routes;
mod startup;
mod storage;

//-----------------------------------------------------------------------------
// Application State - Define ALL shared state components here
//-----------------------------------------------------------------------------
#[derive(Clone)] // Needed for Axum state extension
pub struct AppState {
    // Concrete clients might be needed for direct use or specific configs
    db_client: DynamoDbClient,
    s3_client: S3Client,
    // Trait objects for dependency injection via interfaces
    meme_repo: Arc<dyn MemeRepository>,
    file_storage: Arc<dyn FileStorage>,
    // Shared application configuration
    config: Arc<Config>,
}

//-----------------------------------------------------------------------------
// Main Entry Point
//-----------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), AppError> {
    // --- Initialize Tracing ---
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // Define default log levels if RUST_LOG isn't set
            "axum_meme_posting_example=debug,tower_http=debug,info".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();
    info!("Tracing initialized.");

    // --- Load Configuration ---
    // Load config early, fail fast if required vars are missing
    let config = Config::load().map_err(|e| {
        eprintln!("❌ Configuration Error: {}", e); // Log critical error to stderr
        AppError::InitError(format!("Configuration loading failed: {}", e))
    })?;
    // Config is logged within Config::load now

    // --- Create AWS SDK Config and Clients ---
    info!("Initializing AWS SDK config and clients...");
    let sdk_config = aws_clients::create_sdk_config(&config).await?; // Create base SDK config from App Config

    let db_client = aws_clients::create_dynamodb_client(&sdk_config); // Create DynamoDB client
    let s3_client = aws_clients::create_s3_client(&sdk_config); // Create S3 client
    info!("AWS clients initialized.");

    // --- Initialize AWS Resources (DynamoDB Table, S3 Bucket) ---
    // Ensure backend resources are ready before starting the server
    init_resources(
        &db_client,
        &s3_client,
        &config.dynamodb_table_name, // Pass table name from config
        &config.meme_bucket_name,    // Pass bucket name from config
        &config.aws_region,
    )
    .await?; // Propagate errors
    info!("AWS resources initialized successfully.");

    // --- Create Repository and Storage Implementations ---
    // Instantiate concrete types, passing clients and required config
    let meme_repo_impl = DynamoDbMemeRepository::new(
        db_client.clone(), // Clone client needed for repo
        config.dynamodb_table_name.clone(), // Pass table name
    );
    let file_storage_impl = S3FileStorage::new(
        s3_client.clone(), // Clone client needed for storage
        config.meme_bucket_name.clone(), // Pass bucket name
    );
    info!("Repository and Storage implementations created.");

    // --- Create Application State ---
    // Bundle all shared components into an Arc<AppState>
    let app_state = Arc::new(AppState {
        db_client, // Move clients into state
        s3_client,
        // Convert concrete impls to trait objects (Arc<dyn Trait>)
        meme_repo: Arc::new(meme_repo_impl),
        file_storage: Arc::new(file_storage_impl),
        // Share config using Arc
        config: Arc::new(config),
    });
    info!("Application state created.");

    // --- Create Router ---
    let app = create_router(app_state.clone()); // Pass Arc<AppState> to router setup
    info!("Axum router created.");

    // --- Start Server ---
    let bind_address = app_state.config.bind_address; // Get bind address from config in state
    info!("Server listening on http://{}", bind_address);
    let listener = tokio::net::TcpListener::bind(bind_address)
        .await
        .map_err(|e| AppError::InitError(format!("Failed to bind to address {}: {}", bind_address, e)))?;

    // Run the server with graceful shutdown
    axum::serve(listener, app.into_make_service()) // Use app directly if using Axum 0.7+
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| AppError::InternalServerError(format!("Server execution failed: {}", e)))?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>(); // Pending forever on non-Unix

    tokio::select! {
        _ = ctrl_c => { info!("Received Ctrl+C, shutting down gracefully...")},
        _ = terminate => { info!("Received SIGTERM, shutting down gracefully...")},
    }
}
