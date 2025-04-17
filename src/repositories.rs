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
use tracing::{self, info};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DynamoDbMemeRepository {
    client: DynamoDbClient,
    table_name: String, // Store the table name
}

impl DynamoDbMemeRepository {
    /// Creates a new repository instance configured for a specific table.
    pub fn new(client: DynamoDbClient, table_name: String) -> Self {
        info!(%table_name, "Initializing DynamoDbMemeRepository");
        Self { client, table_name }
    }
}

#[async_trait]
impl MemeRepository for DynamoDbMemeRepository {
    /// Stores a `Meme` in the DynamoDB table using PutItem.
    async fn create(&self, meme: &Meme) -> Result<(), RepoError> {
        self.client
            .put_item()
            .table_name(&self.table_name) // Use stored table name
            .item("meme_id", AttributeValue::S(meme.meme_id.to_string()))
            .item("title", AttributeValue::S(meme.title.clone()))
            .item("description", AttributeValue::S(meme.description.clone()))
            .item("image_key", AttributeValue::S(meme.image_key.clone()))
            .send()
            .await
            .context(format!("DynamoDB (table: {}): Failed to put meme (id: {})", self.table_name, meme.meme_id))
            .map_err(RepoError::BackendError)?; // Map anyhow::Error -> RepoError
        Ok(())
    }

    /// Retrieves a `Meme` from DynamoDB using GetItem.
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Meme>, RepoError> {
        let id_str = id.to_string();
        let resp = self.client
            .get_item()
            .table_name(&self.table_name) // Use stored table name
            .key("meme_id", AttributeValue::S(id_str.clone()))
            .send()
            .await
            .context(format!("DynamoDB (table: {}): Failed to get meme (id: {})", self.table_name, id_str))
            .map_err(RepoError::BackendError)?;

        match resp.item {
            Some(item) => match item_to_meme(&item) {
                Some(meme) => Ok(Some(meme)),
                None => {
                    tracing::error!(meme_id = %id_str, table_name = %self.table_name, "DynamoDB: Retrieved item but failed to parse into Meme");
                    // Return a RepoError indicating data inconsistency
                    Err(RepoError::DataCorruption(format!(
                        "Failed to parse meme data retrieved from DynamoDB table '{}' for id {}",
                        self.table_name, id_str
                    )))
                }
            },
            None => Ok(None), // Item not found is not an error
        }
    }

    /// Lists all memes using DynamoDB Scan. Handles pagination.
    async fn list_all(&self) -> Result<Vec<Meme>, RepoError> {
        tracing::debug!("DynamoDB: Scanning table '{}' for all memes", self.table_name);
        let mut memes: Vec<Meme> = Vec::new();
        let mut last_evaluated_key: Option<HashMap<String, AttributeValue>> = None;

        loop {
            let mut request_builder = self.client.scan().table_name(&self.table_name); // Use stored table name

            // Apply ExclusiveStartKey if paginating from previous response
            if let Some(lek) = last_evaluated_key {
                request_builder = request_builder.set_exclusive_start_key(Some(lek));
            }

            let resp = request_builder
                .send()
                .await
                .context(format!("DynamoDB: Failed to scan table '{}'", self.table_name))
                .map_err(RepoError::BackendError)?;

            if let Some(items) = resp.items {
                tracing::debug!("DynamoDB Scan (table: {}): Returned {} items", self.table_name, items.len());
                for item in items {
                    match item_to_meme(&item) {
                        Some(meme) => memes.push(meme),
                        None => {
                            let item_id = item.get("meme_id").and_then(|v| v.as_s().ok());
                            tracing::error!(item.id = ?item_id, table_name = %self.table_name, "DynamoDB: Failed to parse item from scan into Meme");
                            // Fail fast if data in the table is corrupt
                            return Err(RepoError::DataCorruption(format!(
                                "DynamoDB: Failed to parse item {:?} during scan of table '{}'",
                                item_id, self.table_name
                            )));
                        }
                    }
                }
            } else {
                tracing::debug!("DynamoDB Scan (table: {}): Returned no items in this page.", self.table_name);
            }

            // Check for next page
            last_evaluated_key = resp.last_evaluated_key;
            if last_evaluated_key.is_none() {
                tracing::debug!("DynamoDB Scan (table: {}): Complete.", self.table_name);
                break; // Exit loop if no more pages
            } else {
                tracing::debug!("DynamoDB Scan (table: {}): Continuing with LastEvaluatedKey...", self.table_name);
            }
        }

        tracing::info!("DynamoDB (table: {}): Successfully listed {} memes", self.table_name, memes.len());
        Ok(memes)
    }

    /// Deletes an item from DynamoDB using DeleteItem.
    async fn delete(&self, id: Uuid) -> Result<(), RepoError> {
        let id_str = id.to_string();
        tracing::debug!(meme_id = %id_str, table_name = %self.table_name, "DynamoDB: Deleting item");

        self.client
            .delete_item()
            .table_name(&self.table_name) // Use stored table name
            .key("meme_id", AttributeValue::S(id_str.clone()))
            // DeleteItem succeeds even if item not found, so no need for ConditionExpression unless required
            .send()
            .await
            .context(format!("DynamoDB (table: {}): Failed to delete meme (id: {})", self.table_name, id_str))
            .map_err(RepoError::BackendError)?;

        tracing::debug!(meme_id = %id_str, table_name = %self.table_name, "DynamoDB: Delete request sent");
        Ok(())
    }
}

// Helper function to convert DynamoDB item map to Meme struct
// Remains internal to this module.
fn item_to_meme(item: &HashMap<String, AttributeValue>) -> Option<Meme> {
    // Use flat_map style for conciseness and early exit on None/Err
    let meme_id = item
        .get("meme_id")?
        .as_s()
        .ok()
        .and_then(|s| Uuid::parse_str(s).ok())?;
    let title = item.get("title")?.as_s().ok()?.to_string();
    let description = item.get("description")?.as_s().ok()?.to_string();
    let image_key = item.get("image_key")?.as_s().ok()?.to_string();

    Some(Meme {
        meme_id,
        title,
        description,
        image_key,
    })
}
