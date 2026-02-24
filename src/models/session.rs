use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScanSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub active_container_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub items_scanned: i32,
    pub items_created: i32,
    pub items_moved: i32,
}

/// A single event in a stocker batch submission.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum StockerBatchEvent {
    #[serde(rename = "set_context")]
    SetContext {
        barcode: String,
        scanned_at: DateTime<Utc>,
    },
    #[serde(rename = "move_item")]
    MoveItem {
        barcode: String,
        coordinate: Option<serde_json::Value>,
        scanned_at: DateTime<Utc>,
    },
    #[serde(rename = "create_and_place")]
    CreateAndPlace {
        barcode: String,
        name: Option<String>,
        description: Option<String>,
        category: Option<String>,
        tags: Option<Vec<String>>,
        is_container: Option<bool>,
        coordinate: Option<serde_json::Value>,
        condition: Option<String>,
        metadata: Option<serde_json::Value>,
        scanned_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct StockerBatchRequest {
    pub events: Vec<StockerBatchEvent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockerBatchResponse {
    pub processed: usize,
    pub results: Vec<StockerBatchResult>,
    pub errors: Vec<StockerBatchError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum StockerBatchResult {
    ContextSet {
        index: usize,
        status: String,
        context_set: String,
    },
    Moved {
        index: usize,
        status: String,
        event_id: Uuid,
    },
    Created {
        index: usize,
        status: String,
        event_id: Uuid,
        item_id: Uuid,
        needs_details: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct StockerBatchError {
    pub index: usize,
    pub code: String,
    pub message: String,
}
