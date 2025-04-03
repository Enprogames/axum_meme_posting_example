use axum::{
    extract::{Multipart, Path, State},
    routing::{get, post},
    Router, Json,
};
use tower_http::cors::CorsLayer;
use uuid::Uuid;
use crate::models::Meme;
use aws_clients::{create_dynamodb_client, create_s3_client};
use aws_sdk_s3::Client as S3Client;
use std::sync::Arc;
use hyper::Server;

// Bring the trait into scope so that the generated methods are available.
use modyne::Table;

mod aws_clients;
mod models;

#[derive(Clone)]
struct AppState {
    db_client: aws_sdk_dynamodb::Client,
    s3_client: S3Client,
    bucket: String,
}

#[tokio::main]
async fn main() {
    // Create the DynamoDB client.
    let db_client = create_dynamodb_client().await;
    
    // Create the DynamoDB table if it doesn't exist.
    // (The derive macro on `Meme` should generate this method.)
    Meme::create_table_if_not_exists(&db_client).await.unwrap();

    let s3_client = create_s3_client().await;
    let bucket = "memes-bucket".to_string();

    // Create the S3 bucket.
    let _ = s3_client
        .create_bucket()
        .bucket(&bucket)
        .send()
        .await;

    let state = Arc::new(AppState {
        db_client,
        s3_client,
        bucket,
    });

    let app = Router::new()
        .route("/upload_meme", post(upload_meme))
        .route("/meme/:id", get(get_meme))
        .layer(CorsLayer::permissive())
        .with_state(state);

    Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn upload_meme(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Json<Meme> {
    let meme_id = Uuid::new_v4();
    let mut title = String::new();
    let mut description = String::new();
    let mut image_data = Vec::new();

    while let Some(field) = multipart.next_field().await.unwrap() {
        match field.name().unwrap() {
            "title" => title = field.text().await.unwrap(),
            "description" => description = field.text().await.unwrap(),
            "image" => image_data = field.bytes().await.unwrap().to_vec(),
            _ => (),
        }
    }

    let image_key = format!("{}.png", meme_id);

    // Upload image data to S3.
    state.s3_client
        .put_object()
        .bucket(&state.bucket)
        .key(&image_key)
        .body(image_data.into())
        .send()
        .await
        .unwrap();

    let meme = Meme {
        meme_id,
        title,
        description,
        image_key,
    };

    // Store the meme metadata using the Modyne-generated method.
    Meme::put_item(&state.db_client, &meme).await.unwrap();

    Json(meme)
}

async fn get_meme(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Json<Meme> {
    let meme = Meme::get_item(&state.db_client, id)
        .await
        .unwrap()
        .unwrap();
    Json(meme)
}
