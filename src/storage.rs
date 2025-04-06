use crate::{
    domain::FileStorage,
    errors::StorageError,
};
use anyhow::Context;
use async_trait::async_trait;
use aws_sdk_s3::{
    primitives::ByteStream,
    Client as S3Client,
    error::SdkError,
};
use tracing;

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
    /// Uploads data to S3 using PutObject. Sets Content-Type.
    async fn upload(&self, key: &str, data: Vec<u8>, content_type: Option<String>) -> Result<(), StorageError> {
        let ct_log = content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string()); // Clone for logging if needed, or use ? directly
        tracing::debug!(s3_key = %key, bucket = %self.bucket_name, content_type = ?content_type, "S3: Uploading file");

        let body = ByteStream::from(data);
        self.client
            .put_object()
            .bucket(&self.bucket_name)
            .key(key)
            .body(body)
            // --- Set the Content-Type metadata on the S3 object ---
            .content_type(ct_log)
            // ------------------------------------------------------
            .send()
            .await
            .context(format!("S3: Failed to upload object with key '{}'", key))
            .map_err(|e| StorageError::UploadFailed(e.to_string()))?; // Map to specific upload error

        tracing::debug!(s3_key = %key, bucket = %self.bucket_name, "S3: Upload successful");
        Ok(())
    }

    /// Downloads file data and its content type from S3 using GetObject.
    async fn download(&self, key: &str) -> Result<(ByteStream, Option<String>), StorageError> {
        tracing::debug!(s3_key = %key, bucket = %self.bucket_name, "S3: Downloading file");

        let output = self.client
            .get_object()
            .bucket(&self.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|sdk_err| { // Map SdkError
                // Check specifically for NoSuchKey
                if let SdkError::ServiceError(service_err) = &sdk_err {
                    if service_err.err().meta().code() == Some("NoSuchKey") {
                         tracing::warn!(s3_key = %key, bucket = %self.bucket_name, "S3: NoSuchKey error downloading file");
                         return StorageError::NotFound(key.to_string()); // Return specific NotFound error
                    }
                }
                // For other errors, wrap them in BackendError
                tracing::error!(s3_key = %key, bucket = %self.bucket_name, error = %sdk_err, "S3: Error downloading file");
                StorageError::BackendError(anyhow::Error::new(sdk_err).context(format!("S3: Failed to download object with key '{}'", key)))
            })?;

        let content_type = output.content_type().map(|s| s.to_string());
        tracing::debug!(s3_key = %key, bucket = %self.bucket_name, ?content_type, "S3: Download successful");

        // output.body is the ByteStream
        Ok((output.body, content_type))
    }

    /// Deletes an object from S3 using DeleteObject.
    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        tracing::debug!(s3_key = %key, bucket = %self.bucket_name, "S3: Deleting object");

        self.client
            .delete_object()
            .bucket(&self.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|sdk_err| { // Map SdkError
                // DeleteObject generally succeeds even if the object doesn't exist.
                // We only really care about actual backend/permission errors here.
                tracing::error!(s3_key = %key, bucket = %self.bucket_name, error = %sdk_err, "S3: Error deleting object");
                StorageError::BackendError(anyhow::Error::new(sdk_err).context(format!("S3: Failed to delete object with key '{}'", key)))
            })?;

        tracing::debug!(s3_key = %key, bucket = %self.bucket_name, "S3: Delete request successful (object might not have existed)");
        Ok(())
    }
}
