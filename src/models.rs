use modyne::Table;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Table, Serialize, Deserialize, Debug, Clone)]
#[modyne(table_name = "memes", partition_key = "meme_id")]
pub struct Meme {
    pub meme_id: Uuid,
    pub title: String,
    pub description: String,
    pub image_key: String,
}
