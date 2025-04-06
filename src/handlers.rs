use crate::{
    errors::{AppError, StorageError},
    models::Meme,
    AppState,
};
use axum::{
    body::Body,
    // ----------------------------------------------------
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use mime_guess;
use std::sync::Arc;
use tracing;
use uuid::Uuid;

// upload_meme handler remains mostly the same...
pub async fn upload_meme(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
     let meme_id = Uuid::new_v4();
    let mut title = None;
    let mut description = None;
    let mut image_data: Option<Vec<u8>> = None;
    let mut image_filename: Option<String> = None;
    let mut image_content_type: Option<String> = None;

    while let Some(field) = multipart.next_field().await? {
        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => continue,
        };
        match field_name.as_str() {
            "title" => title = Some(field.text().await.map_err(|e| AppError::InvalidInput(format!("Failed to read title: {}", e)))?),
            "description" => description = Some(field.text().await.map_err(|e| AppError::InvalidInput(format!("Failed to read description: {}", e)))?),
            "image" => {
                image_filename = field.file_name().map(|s| s.to_string());
                image_content_type = field.content_type().map(|m| m.to_string());
                image_data = Some(field.bytes().await?.to_vec());
            }
            _ => tracing::debug!("Ignoring unknown multipart field: {}", field_name),
        }
    }

    let title = title.ok_or_else(|| AppError::MissingFormField("title".to_string()))?;
    let description = description.ok_or_else(|| AppError::MissingFormField("description".to_string()))?;
    let image_data = image_data.ok_or_else(|| AppError::MissingFormField("image".to_string()))?;
    if image_data.is_empty() {
        return Err(AppError::InvalidInput("image data cannot be empty".to_string()));
    }

    let extension = image_filename.as_ref()
        .and_then(|name| name.split('.').last().map(|ext| ext.to_lowercase()))
        .unwrap_or_else(|| "bin".to_string());
    let image_key = format!("{}.{}", meme_id, extension);

    // Guess content type more reliably for upload if not provided
    let final_content_type = image_content_type
         .or_else(|| mime_guess::from_path(&image_key).first_raw().map(|s| s.to_string()))
         .unwrap_or_else(|| "application/octet-stream".to_string());

    // Use the FileStorage trait object from state
    // Pass the determined content type
    state.file_storage
         .upload(&image_key, image_data, Some(final_content_type))
         .await?;

    // Create and Store Meme Metadata
    let meme = Meme {
        meme_id,
        title,
        description,
        image_key,
    };
    state.meme_repo.create(&meme).await?;

    tracing::info!(meme_id = %meme_id, "Meme created successfully via handler");
    Ok((StatusCode::CREATED, Json(meme)))
}


// get_meme handler remains the same...
pub async fn get_meme(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let meme_id = Uuid::parse_str(&id_str)?;
    tracing::debug!(%meme_id, "Fetching meme details via handler");
    let maybe_meme = state.meme_repo.get_by_id(meme_id).await?;
    match maybe_meme {
        Some(meme) => Ok(Json(meme)),
        None => Err(AppError::MemeNotFound(meme_id)),
    }
}

// list_memes handler remains the same...
pub async fn list_memes(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    tracing::debug!("Listing all memes via handler");
    let memes = state.meme_repo.list_all().await?;
    tracing::info!("Handler successfully retrieved {} memes", memes.len());
    Ok(Json(memes))
}


/// Handler for GET /images/{key}
pub async fn get_image(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Result<Response, AppError> {
    tracing::debug!(image_key = %key, "Fetching image file via handler");

    let (byte_stream, content_type) = state.file_storage.download(&key).await?;

    let content_type_header = content_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    // --- WORKAROUND: Collect the stream into memory ---
    let data = byte_stream
        .collect() // Consume the stream
        .await
        .map_err(|e| AppError::StorageError(StorageError::BackendError(anyhow::Error::new(e).context("Failed to collect image bytes from storage"))))?; // Map SDK error

    // Convert AggregatedBytes to axum's Bytes type
    let bytes = data.into_bytes();

    // Create Axum body from the collected bytes
    let body = Body::from(bytes);
    // -------------------------------------------------

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type_header)
        .body(body) // Use body created from collected Bytes
        .map_err(|e| AppError::InternalServerError(format!("Failed to build image response: {}", e)))?;

    Ok(response)
}


/// Deletes the meme metadata and its corresponding image file.
pub async fn delete_meme(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
) -> Result<StatusCode, AppError> { // Return only status code on success
    // Validate UUID format
    let meme_id = Uuid::parse_str(&id_str)?;
    tracing::debug!(%meme_id, "Deleting meme via handler");

    // 1. Get the meme metadata first to ensure it exists and to get the image_key
    let meme_to_delete = state.meme_repo.get_by_id(meme_id).await? // -> RepoError -> AppError
        .ok_or(AppError::MemeNotFound(meme_id))?; // If None, map to AppError::MemeNotFound (404)

    // 2. Delete the image file from S3 storage
    // We proceed even if S3 delete fails for "not found", but fail on other errors.
    match state.file_storage.delete(&meme_to_delete.image_key).await {
        Ok(_) => {
            tracing::debug!(image_key=%meme_to_delete.image_key, "Successfully deleted image from storage (or it was already gone).")
        },
        Err(StorageError::NotFound(_)) => { // Or maybe don't even have NotFound for delete
             tracing::warn!(image_key=%meme_to_delete.image_key, "Image file not found in storage during delete, proceeding with metadata deletion.");
        }
        Err(e) => { // Any other storage error (BackendError) is fatal here
            tracing::error!(image_key=%meme_to_delete.image_key, error=?e, "Failed to delete image file from storage.");
            return Err(e.into()); // Convert StorageError to AppError and return
        }
    }

    // 3. Delete the meme metadata from the repository
    state.meme_repo.delete(meme_id).await?; // Propagate RepoError -> AppError

    tracing::info!(%meme_id, "Meme deleted successfully via handler");

    // 4. Return 204 No Content on successful deletion
    Ok(StatusCode::NO_CONTENT)
}
