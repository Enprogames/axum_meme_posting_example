use crate::errors::{RepoError, StorageError};
use crate::models::Meme;
use async_trait::async_trait;
use uuid::Uuid;
use aws_sdk_s3::primitives::ByteStream;

#[async_trait]
pub trait MemeRepository: Send + Sync + 'static {
    async fn create(&self, meme: &Meme) -> Result<(), RepoError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Meme>, RepoError>;
    async fn list_all(&self) -> Result<Vec<Meme>, RepoError>;
    /// Deletes a meme's metadata by its unique ID.
    /// Should typically succeed even if the item doesn't exist, unless there's a backend error.
    async fn delete(&self, id: Uuid) -> Result<(), RepoError>;
}

#[async_trait]
pub trait FileStorage: Send + Sync + 'static {
    async fn upload(&self, key: &str, data: Vec<u8>, content_type: Option<String>) -> Result<(), StorageError>;
    async fn download(&self, key: &str) -> Result<(ByteStream, Option<String>), StorageError>;
    /// Deletes a file by its key.
    /// Should typically succeed even if the file doesn't exist, unless there's a backend error.
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
}
