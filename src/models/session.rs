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
    pub device_id: Option<String>,
    pub items_errored: i32,
    pub notes: Option<String>,
}

/// Optional body for starting a new scan session.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct StartSessionRequest {
    pub device_id: Option<String>,
    pub notes: Option<String>,
    /// UUID of a container to pre-set as the active context at session start.
    pub initial_container_id: Option<Uuid>,
}

/// A single event in a stocker batch submission.
/// `CreateAndPlace` is intentionally large (many optional fields for a rich create operation);
/// it is only deserialized once per HTTP request so stack copying cost is negligible.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum StockerBatchEvent {
    #[serde(rename = "set_context")]
    SetContext {
        container_id: Uuid,
        scanned_at: DateTime<Utc>,
    },
    #[serde(rename = "move_item")]
    MoveItem {
        item_id: Uuid,
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
        // M-4: additional fields exposed for richer batch creation
        is_fungible: Option<bool>,
        fungible_quantity: Option<i32>,
        fungible_unit: Option<String>,
        external_codes: Option<Vec<crate::models::item::ExternalCode>>,
        container_type_id: Option<Uuid>,
    },
    #[serde(rename = "resolve")]
    Resolve {
        barcode: String,
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
#[serde(tag = "type")]
pub enum StockerBatchResult {
    #[serde(rename = "context_set")]
    ContextSet {
        index: usize,
        status: String,
        container_id: Uuid,
    },
    #[serde(rename = "moved")]
    Moved {
        index: usize,
        status: String,
        event_id: Uuid,
    },
    #[serde(rename = "created")]
    Created {
        index: usize,
        status: String,
        event_id: Uuid,
        item_id: Uuid,
        needs_details: bool,
    },
    #[serde(rename = "resolved")]
    Resolved {
        index: usize,
        status: String,
        resolution: crate::models::barcode::BarcodeResolution,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct StockerBatchError {
    pub index: usize,
    pub code: String,
    pub message: String,
}
