// Standard library imports
use std::collections::HashMap;

// External crate imports
use anyhow::{Context, Result}; // Using anyhow::Result for internal DB functions
use aws_sdk_dynamodb::{
    error::SdkError,
    types::{
        AttributeDefinition, BillingMode, KeySchemaElement, KeyType, ScalarAttributeType,
        AttributeValue, // Keep this import
    },
    Client as DynamoDbClient, // Keep this import
    // Remove unused operation-specific errors if SdkError is handled broadly
};
use tracing; // Keep tracing
use uuid::Uuid;

// Internal crate imports
use crate::models::Meme;

/// The name of the DynamoDB table used for memes.
pub const MEMES_TABLE: &str = "memes";

/// Creates the DynamoDB table for storing memes, if it does not already exist.
///
/// The table uses `meme_id` as the partition (hash) key and PayPerRequest billing.
pub async fn create_memes_table(client: &DynamoDbClient) -> Result<()> {
    let result = client
        .create_table()
        .table_name(MEMES_TABLE)
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("meme_id")
                .attribute_type(ScalarAttributeType::S) // UUID stored as String
                .build()
                .context("Failed to build attribute definition for meme_id")?, // Handle BuildError
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("meme_id")
                .key_type(KeyType::Hash) // Partition key
                .build()
                .context("Failed to build key schema for meme_id")?, // Handle BuildError
        )
        .billing_mode(BillingMode::PayPerRequest)
        .send()
        .await;

    match result {
        Ok(_) => {
            tracing::info!("Table '{}' created successfully or already existed.", MEMES_TABLE);
            Ok(())
        }
        Err(e) => {
            // Check if the error is specifically that the table already exists
            if let SdkError::ServiceError(service_err) = &e {
                if service_err.err().is_resource_in_use_exception() {
                    tracing::info!("Table '{}' already exists, no action needed.", MEMES_TABLE);
                    Ok(()) // Not an error in our context if it exists
                } else {
                    // Different service error
                     Err(anyhow::Error::new(e).context(format!(
                        "Service error creating DynamoDB table '{}'",
                        MEMES_TABLE
                    )))
                }
            } else {
                 // Other SDK errors (dispatch, timeout, etc.)
                 Err(anyhow::Error::new(e).context(format!(
                    "SDK error creating DynamoDB table '{}'",
                    MEMES_TABLE
                )))
            }
        }
    }
}


/// Converts a DynamoDB item (a HashMap) into a `Meme` instance.
/// Returns `None` if any required field is missing or has the wrong type,
/// or if the meme_id is not a valid UUID.
fn item_to_meme(item: &HashMap<String, AttributeValue>) -> Option<Meme> { // Take reference
    // Use .get() and .as_s() which return Option<&String>, then .ok()? to propagate None
    let meme_id_str = item.get("meme_id")?.as_s().ok()?;
    let title = item.get("title")?.as_s().ok()?;
    let description = item.get("description")?.as_s().ok()?;
    let image_key = item.get("image_key")?.as_s().ok()?;

    // Attempt to parse the UUID string
    let meme_id = Uuid::parse_str(meme_id_str).ok()?;

    Some(Meme {
        meme_id,
        title: title.to_string(), // Clone String from &str
        description: description.to_string(),
        image_key: image_key.to_string(),
    })
}

/// Stores a `Meme` in the DynamoDB table.
///
/// This function uses the PutItem builder pattern.
/// It adds context to potential errors using `anyhow`.
pub async fn put_meme(client: &DynamoDbClient, meme: &Meme) -> Result<()> {
    client
        .put_item()
        .table_name(MEMES_TABLE)
        // Build attributes directly
        .item("meme_id", AttributeValue::S(meme.meme_id.to_string()))
        .item("title", AttributeValue::S(meme.title.clone()))
        .item("description", AttributeValue::S(meme.description.clone()))
        .item("image_key", AttributeValue::S(meme.image_key.clone()))
        .send()
        .await
        .context(format!("Failed to put meme (id: {}) metadata in DynamoDB", meme.meme_id))?; // Add context
    Ok(())
}

/// Retrieves a `Meme` from DynamoDB using the given `meme_id`.
///
/// Returns:
/// - `Ok(Some(Meme))` if found,
/// - `Ok(None)` if not found,
/// - `Err(anyhow::Error)` if the AWS SDK operation fails or item data is invalid.
pub async fn get_meme(client: &DynamoDbClient, meme_id: &str) -> Result<Option<Meme>> {
    // Validate UUID format *before* making the AWS call
    if Uuid::parse_str(meme_id).is_err() {
        tracing::warn!(invalid_meme_id = %meme_id, "Attempted to get meme with invalid UUID format");
        // Return Ok(None) because the *item* won't be found with an invalid ID format,
        // rather than indicating a server error. Or return an InvalidInput error.
        // Let's return None for simplicity here, handler can map to 404.
        return Ok(None);
    }

    let resp = client
        .get_item()
        .table_name(MEMES_TABLE)
        .key("meme_id", AttributeValue::S(meme_id.to_string()))
        .send()
        .await
        .context(format!("Failed to get meme (id: {}) from DynamoDB", meme_id))?;

    match resp.item {
        Some(item) => {
            // Attempt to convert the retrieved item into a Meme struct
             match item_to_meme(&item) { // Pass reference
                Some(meme) => Ok(Some(meme)),
                None => {
                    // Item found, but parsing failed (data corruption?)
                    tracing::error!(meme_id = %meme_id, "Retrieved item from DynamoDB but failed to parse it into a Meme struct");
                    // Indicate an internal issue rather than just "not found"
                    Err(anyhow::anyhow!("Failed to parse meme data retrieved from DynamoDB for id {}", meme_id))
                }
            }
        }
        None => {
            // Item not found in DynamoDB
            Ok(None)
        }
    }
}

/// Lists all memes currently stored in the DynamoDB table.
///
/// NOTE: A `Scan` operation reads the entire table, which can be inefficient
/// and costly for large tables. Consider alternative query patterns (e.g., using
/// Global Secondary Indexes) for production use cases if applicable.
/// This implementation does not handle pagination.
///
/// Returns:
/// - `Ok(Vec<Meme>)` containing all valid memes found.
/// - `Err(anyhow::Error)` if the AWS SDK operation fails or parsing any item fails.
pub async fn list_memes(client: &DynamoDbClient) -> Result<Vec<Meme>> {
    tracing::debug!("Scanning DynamoDB table '{}' for all memes", MEMES_TABLE);
    let mut memes: Vec<Meme> = Vec::new();
    let mut last_evaluated_key = None;

    // Basic pagination loop (scan until no more items)
    loop {
         let mut request = client.scan().table_name(MEMES_TABLE);
         if let Some(lek) = last_evaluated_key {
             request = request.set_exclusive_start_key(Some(lek));
         }

         let resp = request
             .send()
             .await
             .context(format!("Failed to scan DynamoDB table '{}'", MEMES_TABLE))?;

         if let Some(items) = resp.items {
             tracing::debug!("Scan returned {} items", items.len());
             for item in items {
                 match item_to_meme(&item) { // Pass reference
                     Some(meme) => memes.push(meme),
                     None => {
                         // Log the issue but continue processing other items
                         // Alternatively, could return an error immediately
                         let item_id = item.get("meme_id").and_then(|v| v.as_s().ok());
                         tracing::error!(item.id = ?item_id, "Failed to parse item from DynamoDB scan into Meme struct");
                         // Optionally return error:
                         // return Err(anyhow::anyhow!("Failed to parse item {:?} during scan", item_id));
                     }
                 }
             }
         } else {
            tracing::debug!("Scan returned no items in this page.");
         }

         // Check if pagination is complete
         if resp.last_evaluated_key.is_none() {
             tracing::debug!("Scan complete. No LastEvaluatedKey found.");
             break; // Exit loop
         } else {
             tracing::debug!("Continuing scan with LastEvaluatedKey...");
             last_evaluated_key = resp.last_evaluated_key;
         }
    }


    tracing::info!("Successfully listed {} memes from DynamoDB", memes.len());
    Ok(memes)
}
