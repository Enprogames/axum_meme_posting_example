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