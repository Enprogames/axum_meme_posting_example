use crate::{
    domain::MemeRepository,
    errors::RepoError,
    models::Meme,
};
use anyhow::Context;
use async_trait::async_trait;
use aws_sdk_dynamodb::{
    types::AttributeValue,
    Client as DynamoDbClient,
};
use std::collections::HashMap;
use tracing;
use uuid::Uuid;

const MEMES_TABLE: &str = "memes";

#[derive(Debug, Clone)]
pub struct DynamoDbMemeRepository {
    client: DynamoDbClient,
}

impl DynamoDbMemeRepository {
    pub fn new(client: DynamoDbClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MemeRepository for DynamoDbMemeRepository {
    /// Stores a `Meme` in the DynamoDB table using PutItem.
    async fn create(&self, meme: &Meme) -> Result<(), RepoError> {
        self.client
            .put_item()
            .table_name(MEMES_TABLE)
            .item("meme_id", AttributeValue::S(meme.meme_id.to_string()))
            .item("title", AttributeValue::S(meme.title.clone()))
            .item("description", AttributeValue::S(meme.description.clone()))
            .item("image_key", AttributeValue::S(meme.image_key.clone()))
            .send()
            .await
            .context(format!("DynamoDB: Failed to put meme (id: {})", meme.meme_id))?;
        Ok(())
    }


     /// Retrieves a `Meme` from DynamoDB using GetItem.
     async fn get_by_id(&self, id: Uuid) -> Result<Option<Meme>, RepoError> {
        let id_str = id.to_string();
        let resp = self.client
            .get_item()
            .table_name(MEMES_TABLE)
            .key("meme_id", AttributeValue::S(id_str.clone()))
            .send()
            .await
            .context(format!("DynamoDB: Failed to get meme (id: {})", id_str))?;

        match resp.item {
            Some(item) => {
                match item_to_meme(&item) {
                    Some(meme) => Ok(Some(meme)),
                    None => {
                        tracing::error!(meme_id = %id_str, "DynamoDB: Retrieved item but failed to parse into Meme");
                        Err(anyhow::anyhow!("Failed to parse meme data retrieved from DynamoDB for id {}", id_str))
                            .map_err(RepoError::BackendError)
                    }
                }
            }
            None => Ok(None),
        }
    }

    /// Lists all memes using DynamoDB Scan. Handles pagination.
    async fn list_all(&self) -> Result<Vec<Meme>, RepoError> {
        tracing::debug!("DynamoDB: Scanning table '{}' for all memes", MEMES_TABLE);
        let mut memes: Vec<Meme> = Vec::new();
        let mut last_evaluated_key: Option<HashMap<String, AttributeValue>> = None;

        loop {
            let mut request_builder = self.client.scan().table_name(MEMES_TABLE);

            // Apply ExclusiveStartKey if paginating
            if let Some(lek) = last_evaluated_key {
                request_builder = request_builder.set_exclusive_start_key(Some(lek));
            }

            let resp = request_builder
                .send() // Execute the scan request
                .await  // Await the future
                .context(format!("DynamoDB: Failed to scan table '{}'", MEMES_TABLE))?;
            // --------------------------------------------------

            if let Some(items) = resp.items {
                tracing::debug!("DynamoDB Scan: Returned {} items", items.len());
                for item in items {
                    match item_to_meme(&item) {
                        Some(meme) => memes.push(meme),
                        None => {
                            let item_id = item.get("meme_id").and_then(|v| v.as_s().ok());
                            tracing::error!(item.id = ?item_id, "DynamoDB: Failed to parse item from scan into Meme");
                            return Err(anyhow::anyhow!("DynamoDB: Failed to parse item {:?} during scan", item_id))
                                    .map_err(RepoError::BackendError);
                        }
                    }
                }
            } else {
                tracing::debug!("DynamoDB Scan: Returned no items in this page.");
            }

            last_evaluated_key = resp.last_evaluated_key;
            if last_evaluated_key.is_none() {
                tracing::debug!("DynamoDB Scan: Complete.");
                break;
            } else {
                tracing::debug!("DynamoDB Scan: Continuing with LastEvaluatedKey...");
            }
        }

        tracing::info!("DynamoDB: Successfully listed {} memes", memes.len());
        Ok(memes)
    }
}

// Helper function to convert DynamoDB item map to Meme struct
// Remains internal to this module.
fn item_to_meme(item: &HashMap<String, AttributeValue>) -> Option<Meme> {
    let meme_id_str = item.get("meme_id")?.as_s().ok()?;
    let title = item.get("title")?.as_s().ok()?;
    let description = item.get("description")?.as_s().ok()?;
    let image_key = item.get("image_key")?.as_s().ok()?;
    let meme_id = Uuid::parse_str(meme_id_str).ok()?;
    Some(Meme {
        meme_id,
        title: title.to_string(),
        description: description.to_string(),
        image_key: image_key.to_string(),
    })
}
