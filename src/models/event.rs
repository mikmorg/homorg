use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A stored event from the event_store table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StoredEvent {
    pub id: i64,
    pub event_id: Uuid,
    pub aggregate_id: Uuid,
    pub aggregate_type: String,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub metadata: serde_json::Value,
    pub actor_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub sequence_number: i64,
}

/// Domain event variants for type-safe event handling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    ItemCreated(ItemCreatedData),
    ItemUpdated(ItemUpdatedData),
    ItemMoved(ItemMovedData),
    ItemMoveReverted(ItemMoveRevertedData),
    ItemDeleted(ItemDeletedData),
    ItemRestored(ItemRestoredData),
    ItemImageAdded(ItemImageAddedData),
    ItemImageRemoved(ItemImageRemovedData),
    ItemExternalCodeAdded(ExternalCodeData),
    ItemExternalCodeRemoved(ExternalCodeData),
    ItemQuantityAdjusted(QuantityAdjustedData),
    ContainerSchemaUpdated(ContainerSchemaUpdatedData),
    BarcodeGenerated(BarcodeGeneratedData),
}

impl DomainEvent {
    /// Returns the event_type discriminator string for DB storage.
    pub fn event_type(&self) -> &'static str {
        match self {
            DomainEvent::ItemCreated(_) => "ItemCreated",
            DomainEvent::ItemUpdated(_) => "ItemUpdated",
            DomainEvent::ItemMoved(_) => "ItemMoved",
            DomainEvent::ItemMoveReverted(_) => "ItemMoveReverted",
            DomainEvent::ItemDeleted(_) => "ItemDeleted",
            DomainEvent::ItemRestored(_) => "ItemRestored",
            DomainEvent::ItemImageAdded(_) => "ItemImageAdded",
            DomainEvent::ItemImageRemoved(_) => "ItemImageRemoved",
            DomainEvent::ItemExternalCodeAdded(_) => "ItemExternalCodeAdded",
            DomainEvent::ItemExternalCodeRemoved(_) => "ItemExternalCodeRemoved",
            DomainEvent::ItemQuantityAdjusted(_) => "ItemQuantityAdjusted",
            DomainEvent::ContainerSchemaUpdated(_) => "ContainerSchemaUpdated",
            DomainEvent::BarcodeGenerated(_) => "BarcodeGenerated",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCreatedData {
    pub system_barcode: String,
    pub ltree_label: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub is_container: bool,
    pub container_path: String,
    pub parent_id: Uuid,
    pub coordinate: Option<serde_json::Value>,
    pub location_schema: Option<serde_json::Value>,
    pub max_capacity_cc: Option<f64>,
    pub max_weight_grams: Option<f64>,
    pub dimensions: Option<serde_json::Value>,
    pub weight_grams: Option<f64>,
    pub is_fungible: bool,
    pub fungible_quantity: Option<i32>,
    pub fungible_unit: Option<String>,
    pub external_codes: Vec<serde_json::Value>,
    pub condition: Option<String>,
    pub acquisition_date: Option<String>,
    pub acquisition_cost: Option<f64>,
    pub current_value: Option<f64>,
    pub depreciation_rate: Option<f64>,
    pub warranty_expiry: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    pub old: serde_json::Value,
    pub new: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemUpdatedData {
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemMovedData {
    pub from_container_id: Option<Uuid>,
    pub to_container_id: Uuid,
    pub from_path: Option<String>,
    pub to_path: String,
    pub coordinate: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemMoveRevertedData {
    pub original_event_id: Uuid,
    pub from_container_id: Uuid,
    pub to_container_id: Option<Uuid>,
    pub from_path: String,
    pub to_path: Option<String>,
    pub coordinate: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDeletedData {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRestoredData {
    pub from_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemImageAddedData {
    pub path: String,
    pub caption: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemImageRemovedData {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCodeData {
    pub code_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantityAdjustedData {
    pub old_qty: Option<i32>,
    pub new_qty: i32,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSchemaUpdatedData {
    pub old_schema: Option<serde_json::Value>,
    pub new_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarcodeGeneratedData {
    pub barcode: String,
    pub assigned_to: Option<Uuid>,
}

/// Metadata attached to events for correlation/causation tracking.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
}
