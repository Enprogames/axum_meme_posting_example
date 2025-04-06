use crate::{
    errors::{internal_error, AppError}, // Use AppError and internal_error macro
    models::Meme,
    AppState, // Use the refactored AppState (defined in main.rs for now)
};
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse, // Needed for return type
    Json,
};
use mime_guess; // For guessing content type
use std::sync::Arc;
use tracing;
use uuid::Uuid;

/// Handler for POST /upload_meme
pub async fn upload_meme(
    State(state): State<Arc<AppState>>, // State contains Arc<dyn ...> traits
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let meme_id = Uuid::new_v4();
    let mut title = None;
    let mut description = None;
    let mut image_data: Option<Vec<u8>> = None;
    let mut image_filename: Option<String> = None;
    let mut image_content_type: Option<String> = None;

    // --- Process Multipart Form Data ---
    while let Some(field) = multipart.next_field().await? { // Propagate multipart errors -> AppError
        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => continue,
        };

        match field_name.as_str() {
            "title" => title = Some(field.text().await.map_err(|e| AppError::InvalidInput(format!("Failed to read title: {}", e)))?),
            "description" => description = Some(field.text().await.map_err(|e| AppError::InvalidInput(format!("Failed to read description: {}", e)))?),
            "image" => {
                image_filename = field.file_name().map(|s| s.to_string());
                // Try to get content type provided by the client
                image_content_type = field.content_type().map(|m| m.to_string());
                image_data = Some(field.bytes().await?.to_vec());
            }
            _ => tracing::debug!("Ignoring unknown multipart field: {}", field_name),
        }
    }

    // --- Validate Input ---
    let title = title.ok_or_else(|| AppError::MissingFormField("title".to_string()))?;
    let description = description.ok_or_else(|| AppError::MissingFormField("description".to_string()))?;
    let image_data = image_data.ok_or_else(|| AppError::MissingFormField("image".to_string()))?;
    if image_data.is_empty() {
        return Err(AppError::InvalidInput("image data cannot be empty".to_string()));
    }

    // --- Prepare for Storage ---
    // Determine extension and S3 key
    let extension = image_filename.as_ref()
        .and_then(|name| name.split('.').last().map(|ext| ext.to_lowercase()))
        .unwrap_or_else(|| "bin".to_string()); // Default to 'bin' if no extension
    let image_key = format!("{}.{}", meme_id, extension);

    // Guess content type if not provided, default to application/octet-stream
     let final_content_type = image_content_type
         .or_else(|| mime_guess::from_path(&image_key).first_raw().map(|s| s.to_string()))
         .unwrap_or_else(|| "application/octet-stream".to_string());


    // --- Upload Image File ---
    // Use the FileStorage trait object from state
    state.file_storage
         .upload(&image_key, image_data, Some(final_content_type)) // Pass content type
         .await?; // Propagates StorageError -> AppError

    // --- Create and Store Meme Metadata ---
    let meme = Meme {
        meme_id,
        title,
        description,
        image_key,
    };
    // Use the MemeRepository trait object from state
    state.meme_repo.create(&meme).await?; // Propagates RepoError -> AppError

    tracing::info!(meme_id = %meme_id, "Meme created successfully via handler");

    // --- Return Success Response ---
    Ok((StatusCode::CREATED, Json(meme)))
}

/// Handler for GET /meme/{id}
pub async fn get_meme(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // Validate UUID format before hitting the repository
    let meme_id = Uuid::parse_str(&id_str)?; // Propagates uuid::Error -> AppError

    tracing::debug!(%meme_id, "Fetching meme details via handler");

    // Use the MemeRepository trait object
    let maybe_meme = state.meme_repo.get_by_id(meme_id).await?; // Propagates RepoError -> AppError

    match maybe_meme {
        Some(meme) => Ok(Json(meme)), // 200 OK with JSON body
        None => Err(AppError::MemeNotFound(meme_id)), // Map None to 404 AppError
    }
}

/// Handler for GET /memes
pub async fn list_memes(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    tracing::debug!("Listing all memes via handler");

    // Use the MemeRepository trait object
    let memes = state.meme_repo.list_all().await?; // Propagates RepoError -> AppError

    tracing::info!("Handler successfully retrieved {} memes", memes.len());
    Ok(Json(memes)) // 200 OK with JSON array
}
