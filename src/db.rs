// Standard library imports
use std::collections::HashMap;

// External crate imports
use anyhow::{Context, Result};
use aws_sdk_dynamodb::{
    error::SdkError,
    // operation::create_table::CreateTableError, // <-- Remove unused import
    types::{
        AttributeDefinition, BillingMode, KeySchemaElement, KeyType, ScalarAttributeType,
        AttributeValue,
    },
    Client as DynamoDbClient,
};
use tracing;
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
                .attribute_type(ScalarAttributeType::S)
                .build()?,
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("meme_id")
                .key_type(KeyType::Hash)
                .build()?,
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
            if let SdkError::ServiceError(service_err) = &e {
                if service_err.err().is_resource_in_use_exception() {
                    tracing::info!("Table '{}' already exists, no action needed.", MEMES_TABLE);
                    Ok(())
                } else {
                     Err(anyhow::Error::new(e).context(format!(
                        "Failed to create DynamoDB table '{}' due to service error",
                        MEMES_TABLE
                    )))
                }
            } else {
                Err(anyhow::Error::new(e).context(format!(
                    "Failed to create DynamoDB table '{}' due to SDK error",
                    MEMES_TABLE
                )))
            }
        }
    }
}

/// Converts a `Meme` instance into a DynamoDB item represented as a HashMap.
fn meme_to_item(_meme: &Meme) -> HashMap<String, AttributeValue> {
    // This function isn't actually used if using the builder pattern below,
    // but keeping it doesn't hurt if you might switch back.
    // If definitely unused, you could remove it and the unused variable warning.
    // For now, silencing the unused `_meme` parameter warning.
    unimplemented!("meme_to_item is likely unused due to builder pattern in put_meme");
}

/// Converts a DynamoDB item (a HashMap) into a `Meme` instance.
/// Returns `None` if any required field is missing or has the wrong type,
/// or if the meme_id is not a valid UUID.
fn item_to_meme(item: HashMap<String, AttributeValue>) -> Option<Meme> {
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

/// Stores a `Meme` in the DynamoDB table.
///
/// This function uses the PutItem builder pattern.
/// It adds context to potential errors using `anyhow`.
pub async fn put_meme(client: &DynamoDbClient, meme: &Meme) -> Result<()> {
    // let item = meme_to_item(meme); // <-- Remove unused variable
    client
        .put_item()
        .table_name(MEMES_TABLE)
        .item("meme_id", AttributeValue::S(meme.meme_id.to_string()))
        .item("title", AttributeValue::S(meme.title.clone()))
        .item("description", AttributeValue::S(meme.description.clone()))
        .item("image_key", AttributeValue::S(meme.image_key.clone()))
        .send()
        .await
        .context(format!("Failed to put meme (id: {}) metadata in DynamoDB", meme.meme_id))?;
    Ok(())
}

/// Retrieves a `Meme` from DynamoDB using the given `meme_id`.
///
/// Returns:
/// - `Ok(Some(Meme))` if found,
/// - `Ok(None)` if not found or if item data is invalid,
/// - `Err(anyhow::Error)` if the AWS SDK operation fails.
pub async fn get_meme(client: &DynamoDbClient, meme_id: &str) -> Result<Option<Meme>> {
    if Uuid::parse_str(meme_id).is_err() {
        tracing::warn!(invalid_meme_id = %meme_id, "Attempted to get meme with invalid UUID format");
        return Ok(None);
    }

    let resp = client
        .get_item()
        .table_name(MEMES_TABLE)
        .key("meme_id", AttributeValue::S(meme_id.to_string()))
        .send()
        .await
        .context(format!("Failed to get meme (id: {}) from DynamoDB", meme_id))?;

    let maybe_item_ref = resp.item.as_ref(); // Get Option<&HashMap<...>>
    let meme_option = maybe_item_ref.and_then(|item_ref| {
        // item_ref is &HashMap<...>
        // item_to_meme needs HashMap<...>, so clone the referenced item
        item_to_meme(item_ref.clone())
    });

    // Now check original resp.item.is_some() safely after the borrow via as_ref()
    if meme_option.is_none() && resp.item.is_some() {
        tracing::error!(meme_id = %meme_id, "Retrieved item from DynamoDB but failed to parse it into a Meme struct");
    }

    Ok(meme_option)
}