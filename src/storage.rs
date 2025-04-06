use crate::{
    domain::FileStorage,
    errors::StorageError,
};
use anyhow::Context;
use async_trait::async_trait;
use aws_sdk_s3::{primitives::ByteStream, Client as S3Client};
use tracing;

/// Implementation of FileStorage using AWS S3.
#[derive(Debug, Clone)]
pub struct S3FileStorage {
    client: S3Client,
    bucket_name: String,
}

impl S3FileStorage {
    pub fn new(client: S3Client, bucket_name: String) -> Self {
        Self { client, bucket_name }
    }
}

#[async_trait]
impl FileStorage for S3FileStorage {
    /// Uploads data to S3 using PutObject.
    async fn upload(&self, key: &str, data: Vec<u8>, content_type: Option<String>) -> Result<(), StorageError> {
        tracing::debug!(s3_key = %key, bucket = %self.bucket_name, content_type = ?content_type, "S3: Uploading file");

        let body = ByteStream::from(data);
        let mut request_builder = self.client
            .put_object()
            .bucket(&self.bucket_name)
            .key(key)
            .body(body);

        // Set Content-Type if provided
        if let Some(ct) = content_type {
             request_builder = request_builder.content_type(ct);
        }

        request_builder
            .send()
            .await
            .context(format!("S3: Failed to upload object with key '{}'", key)) // anyhow context
            .map_err(StorageError::BackendError)?; // Convert anyhow::Error to StorageError

        tracing::debug!(s3_key = %key, bucket = %self.bucket_name, "S3: Upload successful");
        Ok(())
    }
}
