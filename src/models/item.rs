use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Represents the current materialized state of an item (read projection).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Item {
    pub id: Uuid,
    pub system_barcode: String,
    pub node_id: String,

    // Classification
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,

    // Hierarchy
    pub is_container: bool,
    pub container_path: Option<String>, // LTREE stored as text
    pub parent_id: Option<Uuid>,

    // Coordinate within parent
    pub coordinate: Option<serde_json::Value>,

    // Container properties
    pub location_schema: Option<serde_json::Value>,
    pub max_capacity_cc: Option<rust_decimal::Decimal>,
    pub max_weight_grams: Option<rust_decimal::Decimal>,

    // Physical properties
    pub dimensions: Option<serde_json::Value>,
    pub weight_grams: Option<rust_decimal::Decimal>,

    // Fungible
    pub is_fungible: bool,
    pub fungible_quantity: Option<i32>,
    pub fungible_unit: Option<String>,

    // External codes
    pub external_codes: serde_json::Value,

    // Condition & valuation
    pub condition: Option<String>,
    pub acquisition_date: Option<NaiveDate>,
    pub acquisition_cost: Option<rust_decimal::Decimal>,
    pub current_value: Option<rust_decimal::Decimal>,
    pub depreciation_rate: Option<rust_decimal::Decimal>,
    pub warranty_expiry: Option<NaiveDate>,

    // Extensible
    pub metadata: serde_json::Value,
    pub images: serde_json::Value,

    // Audit
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Slim item representation for list/search results.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ItemSummary {
    pub id: Uuid,
    pub system_barcode: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub is_container: bool,
    pub container_path: Option<String>,
    pub parent_id: Option<Uuid>,
    pub condition: Option<String>,
    pub tags: Vec<String>,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Breadcrumb entry for ancestor path display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AncestorEntry {
    pub id: Uuid,
    pub system_barcode: String,
    pub name: Option<String>,
    pub node_id: String,
    pub depth: usize,
}

/// Full item detail with ancestors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDetail {
    #[serde(flatten)]
    pub item: Item,
    pub ancestors: Vec<AncestorEntry>,
}

/// Container statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStats {
    pub child_count: i64,
    pub descendant_count: i64,
    pub total_weight_grams: Option<f64>,
    pub capacity_used_cc: Option<f64>,
    pub max_capacity_cc: Option<f64>,
    pub utilization_pct: Option<f64>,
}

/// Request to create an item.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateItemRequest {
    pub system_barcode: Option<String>, // auto-generate if absent
    pub parent_id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub is_container: Option<bool>,
    pub coordinate: Option<serde_json::Value>,
    pub location_schema: Option<serde_json::Value>,
    pub max_capacity_cc: Option<f64>,
    pub max_weight_grams: Option<f64>,
    pub dimensions: Option<serde_json::Value>,
    pub weight_grams: Option<f64>,
    pub is_fungible: Option<bool>,
    pub fungible_quantity: Option<i32>,
    pub fungible_unit: Option<String>,
    pub external_codes: Option<Vec<ExternalCode>>,
    pub condition: Option<String>,
    pub acquisition_date: Option<NaiveDate>,
    pub acquisition_cost: Option<f64>,
    pub current_value: Option<f64>,
    pub depreciation_rate: Option<f64>,
    pub warranty_expiry: Option<NaiveDate>,
    pub metadata: Option<serde_json::Value>,
}

/// Request to partially update an item.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateItemRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub is_container: Option<bool>,
    pub coordinate: Option<serde_json::Value>,
    pub location_schema: Option<serde_json::Value>,
    pub max_capacity_cc: Option<f64>,
    pub max_weight_grams: Option<f64>,
    pub dimensions: Option<serde_json::Value>,
    pub weight_grams: Option<f64>,
    pub condition: Option<String>,
    pub acquisition_date: Option<NaiveDate>,
    pub acquisition_cost: Option<f64>,
    pub current_value: Option<f64>,
    pub depreciation_rate: Option<f64>,
    pub warranty_expiry: Option<NaiveDate>,
    pub metadata: Option<serde_json::Value>,
}

/// Request to move an item to a different container.
#[derive(Debug, Clone, Deserialize)]
pub struct MoveItemRequest {
    pub container_id: Uuid,
    pub coordinate: Option<serde_json::Value>,
}

/// External identifier (UPC, EAN, ISBN, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCode {
    #[serde(rename = "type")]
    pub code_type: String,
    pub value: String,
}

/// Request to add an external code.
#[derive(Debug, Clone, Deserialize)]
pub struct AddExternalCodeRequest {
    #[serde(rename = "type")]
    pub code_type: String,
    pub value: String,
}

/// Request to adjust fungible quantity.
#[derive(Debug, Clone, Deserialize)]
pub struct AdjustQuantityRequest {
    pub new_quantity: i32,
    pub reason: Option<String>,
}

/// Image metadata stored in the images JSONB array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEntry {
    pub path: String,
    pub caption: Option<String>,
    pub order: i32,
}
