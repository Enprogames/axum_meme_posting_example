use crate::errors::{RepoError, StorageError};
use crate::models::Meme;
use async_trait::async_trait;
use uuid::Uuid;

/// Trait defining operations for storing and retrieving Meme metadata.
#[async_trait]
pub trait MemeRepository: Send + Sync + 'static { // Send+Sync+'static required for Arc<dyn>
    /// Creates or updates a meme's metadata.
    async fn create(&self, meme: &Meme) -> Result<(), RepoError>;

    /// Retrieves a meme's metadata by its unique ID.
    /// Returns Ok(None) if the meme is not found.
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Meme>, RepoError>;

    /// Lists all meme metadata.
    /// WARNING: This can be inefficient on large datasets. Consider pagination.
    async fn list_all(&self) -> Result<Vec<Meme>, RepoError>;

    // Could add delete, update methods etc. later
}

/// Trait defining operations for storing and retrieving file data (meme images).
#[async_trait]
pub trait FileStorage: Send + Sync + 'static {
    /// Uploads file data to the storage backend.
    async fn upload(&self, key: &str, data: Vec<u8>, content_type: Option<String>) -> Result<(), StorageError>;

    // Could add download, delete, get_presigned_url methods etc. later
    // async fn download(&self, key: &str) -> Result<Vec<u8>, StorageError>;
    // async fn delete(&self, key: &str) -> Result<(), StorageError>;
}
