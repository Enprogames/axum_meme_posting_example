use crate::{
    handlers,
    AppState,
};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

/// Creates the Axum router and associates routes with handlers.
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/upload_meme", post(handlers::upload_meme))
        .route("/meme/{id}",
            get(handlers::get_meme)
            .delete(handlers::delete_meme) // Add delete handler
        )
        .route("/memes", get(handlers::list_memes))
        .route("/images/{key}", get(handlers::get_image))
        // Middleware Layers
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any) 
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .with_state(state)
}
