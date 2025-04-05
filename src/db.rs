// Standard library imports
use std::collections::HashMap;

// External crate imports
use anyhow::{Context, Result}; // Use anyhow for error context
use aws_sdk_dynamodb::{
    error::SdkError, // Import SdkError for specific error handling
    operation::create_table::CreateTableError, // Import specific error type
    types::{
        AttributeDefinition, BillingMode, KeySchemaElement, KeyType, ScalarAttributeType,
        AttributeValue, // *** AttributeValue is also under `types` ***
        // ProvisionedThroughput, // Keep if using Provisioned mode
    },
    Client as DynamoDbClient,
};
use tracing; // Import tracing for logging
use uuid::Uuid; // Explicitly import Uuid

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
                .attribute_type(ScalarAttributeType::S) // S for String
                .build()?, // Use ? on build results
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("meme_id")
                .key_type(KeyType::Hash) // HASH key
                .build()?, // Use ? on build results
        )
        // Using PayPerRequest billing mode - simpler for many use cases
        .billing_mode(BillingMode::PayPerRequest)
        // --- Alternatively, use Provisioned Throughput: ---
        // .provisioned_throughput(
        //     ProvisionedThroughput::builder()
        //         .read_capacity_units(1)
        //         .write_capacity_units(1)
        //         .build()? // Use ? on build results
        // )
        .send()
        .await;

    match result {
        Ok(_) => {
            tracing::info!("Table '{}' created successfully or already existed.", MEMES_TABLE);
            Ok(())
        }
        Err(e) => {
            // Check if the error is specifically ResourceInUseException
            // Use `into_service_error` for robust checking instead of string matching
            if let SdkError::ServiceError(service_err) = &e {
                if service_err.err().is_resource_in_use_exception() {
                    tracing::info!("Table '{}' already exists, no action needed.", MEMES_TABLE);
                    Ok(()) // Table already exists, which is fine.
                } else {
                    // Different service error, propagate it using anyhow
                    Err(anyhow::Error::new(e).context(format!(
                        "Failed to create DynamoDB table '{}' due to service error",
                        MEMES_TABLE
                    )))
                }
            } else {
                // Not a service error (e.g., network issue), propagate it using anyhow
                Err(anyhow::Error::new(e).context(format!(
                    "Failed to create DynamoDB table '{}' due to SDK error",
                    MEMES_TABLE
                )))
            }
        }
    }
}

/// Converts a `Meme` instance into a DynamoDB item represented as a HashMap.
fn meme_to_item(meme: &Meme) -> HashMap<String, AttributeValue> {
    HashMap::from([
        (
            "meme_id".to_string(),
            AttributeValue::S(meme.meme_id.to_string()),
        ),
        ("title".to_string(), AttributeValue::S(meme.title.clone())),
        (
            "description".to_string(),
            AttributeValue::S(meme.description.clone()),
        ),
        (
            "image_key".to_string(),
            AttributeValue::S(meme.image_key.clone()),
        ),
    ])
}

/// Converts a DynamoDB item (a HashMap) into a `Meme` instance.
/// Returns `None` if any required field is missing or has the wrong type,
/// or if the meme_id is not a valid UUID.
fn item_to_meme(item: HashMap<String, AttributeValue>) -> Option<Meme> {
    // Use map and ok_or/ok_or_else for cleaner Option handling if desired,
    // but .get()?.as_s().ok()? is also clear and concise here.
    let meme_id_str = item.get("meme_id")?.as_s().ok()?;
    let title = item.get("title")?.as_s().ok()?;
    let description = item.get("description")?.as_s().ok()?;
    let image_key = item.get("image_key")?.as_s().ok()?;

    // Parse the UUID string, returning None if parsing fails
    let meme_id = Uuid::parse_str(meme_id_str).ok()?;

    Some(Meme {
        meme_id, // Use the parsed Uuid
        title: title.to_string(),
        description: description.to_string(),
        image_key: image_key.to_string(),
    })
}

/// Stores a `Meme` in the DynamoDB table.
///
/// This function converts the `Meme` into an item and calls PutItem.
/// It adds context to potential errors using `anyhow`.
pub async fn put_meme(client: &DynamoDbClient, meme: &Meme) -> Result<()> {
    let item = meme_to_item(meme);
    client
        .put_item()
        .table_name(MEMES_TABLE)
        // .set_item(Some(item)) // `set_item` takes Option<HashMap<..>>
        .item("meme_id", AttributeValue::S(meme.meme_id.to_string())) // More idiomatic builder pattern
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
/// - `Ok(None)` if not found or if item data is invalid,
/// - `Err(anyhow::Error)` if the AWS SDK operation fails.
pub async fn get_meme(client: &DynamoDbClient, meme_id: &str) -> Result<Option<Meme>> {
    // Validate meme_id format early (optional but good practice)
    if Uuid::parse_str(meme_id).is_err() {
        tracing::warn!(invalid_meme_id = %meme_id, "Attempted to get meme with invalid UUID format");
        // Return Ok(None) because the ID format guarantees it won't be found.
        // Or return an Err if you consider this a client error.
        return Ok(None);
        // Alternatively: return Err(anyhow::anyhow!("Invalid meme_id format: {}", meme_id));
    }

    let resp = client
        .get_item()
        .table_name(MEMES_TABLE)
        // Use the builder pattern for keys too
        .key("meme_id", AttributeValue::S(meme_id.to_string()))
        // .set_key(Some(key)) // Alternative using HashMap
        .send()
        .await
        .context(format!("Failed to get meme (id: {}) from DynamoDB", meme_id))?; // Add context

    // Attempt to convert the item (if found) into a Meme
    // `resp.item` is Option<HashMap<String, AttributeValue>>
    // `item_to_meme` converts HashMap to Option<Meme>
    // So we use `and_then` to chain the Option results.
    let meme_option = resp.item.and_then(item_to_meme);

    if meme_option.is_none() && resp.item.is_some() {
        // Log if an item was retrieved but couldn't be converted
        tracing::error!(meme_id = %meme_id, "Retrieved item from DynamoDB but failed to parse it into a Meme struct");
    }

    Ok(meme_option)
}