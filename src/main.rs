use crate::{
    aws_clients::{create_dynamodb_client, create_s3_client},
    config::Config,
    domain::{FileStorage, MemeRepository},
    errors::AppError,
    repositories::DynamoDbMemeRepository,
    routes::create_router,
    startup::init_resources,
    storage::S3FileStorage,
};
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
// Application State (Now uses trait objects)
//-----------------------------------------------------------------------------
#[derive(Clone)]
pub struct AppState {
    meme_repo: Arc<dyn MemeRepository>,
    file_storage: Arc<dyn FileStorage>,
}

//-----------------------------------------------------------------------------
// Main Entry Point
//-----------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "axum_meme_posting_example=debug,tower_http=debug,info".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("Tracing initialized.");

    let config = Config::load()?;
    tracing::info!("Configuration loaded: {:?}", config);

    // --- AWS Client Initialization ---
    // Note: aws_clients::load_config implicitly uses env vars or defaults.
    // Could modify aws_clients to explicitly take config.localstack_endpoint etc.
    tracing::info!("Initializing AWS clients...");
    let db_client = create_dynamodb_client().await;
    let s3_client = create_s3_client().await;
    tracing::info!("AWS clients initialized.");

    init_resources(&db_client, &s3_client, &config.meme_bucket_name, &config.aws_region).await?;

    let meme_repo: Arc<dyn MemeRepository> = Arc::new(DynamoDbMemeRepository::new(db_client));
    let file_storage: Arc<dyn FileStorage> = Arc::new(S3FileStorage::new(s3_client, config.meme_bucket_name.clone()));
    tracing::info!("Repository and Storage implementations created.");

    let app_state = Arc::new(AppState {
        meme_repo,
        file_storage,
    });
    tracing::info!("Application state created.");

    let app = create_router(app_state);
    tracing::info!("Axum router created.");

    let addr = config.bind_address;
    tracing::info!("Server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::InitError(format!("Failed to bind to address {}: {}", addr, e)))?;

    axum::serve(listener, app.into_make_service())
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
