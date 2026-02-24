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
    pub node_id: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: serialise then deserialise a DomainEvent round-trip.
    fn roundtrip(event: &DomainEvent) -> DomainEvent {
        let json = serde_json::to_value(event).expect("serialize");
        serde_json::from_value(json).expect("deserialize")
    }

    #[test]
    fn item_created_serde_roundtrip() {
        let evt = DomainEvent::ItemCreated(ItemCreatedData {
            system_barcode: "HOM-000001".into(),
            node_id: "n_aabbccdd0011".into(),
            name: Some("Widget".into()),
            description: None,
            category: Some("tools".into()),
            tags: vec!["red".into()],
            is_container: false,
            container_path: "n_root.n_abc".into(),
            parent_id: Uuid::nil(),
            coordinate: None,
            location_schema: None,
            max_capacity_cc: None,
            max_weight_grams: None,
            dimensions: None,
            weight_grams: Some(42.0),
            is_fungible: false,
            fungible_quantity: None,
            fungible_unit: None,
            external_codes: vec![],
            condition: None,
            acquisition_date: None,
            acquisition_cost: None,
            current_value: None,
            depreciation_rate: None,
            warranty_expiry: None,
            metadata: serde_json::json!({}),
        });
        let rt = roundtrip(&evt);
        assert_eq!(rt.event_type(), "ItemCreated");
    }

    #[test]
    fn item_updated_serde_roundtrip() {
        let evt = DomainEvent::ItemUpdated(ItemUpdatedData {
            changes: vec![FieldChange {
                field: "name".into(),
                old: serde_json::json!("old"),
                new: serde_json::json!("new"),
            }],
        });
        let rt = roundtrip(&evt);
        assert_eq!(rt.event_type(), "ItemUpdated");
    }

    #[test]
    fn item_moved_serde_roundtrip() {
        let evt = DomainEvent::ItemMoved(ItemMovedData {
            from_container_id: Some(Uuid::nil()),
            to_container_id: Uuid::nil(),
            from_path: Some("n_root".into()),
            to_path: "n_root.n_abc".into(),
            coordinate: None,
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemMoved");
    }

    #[test]
    fn item_move_reverted_serde_roundtrip() {
        let evt = DomainEvent::ItemMoveReverted(ItemMoveRevertedData {
            original_event_id: Uuid::new_v4(),
            from_container_id: Uuid::nil(),
            to_container_id: None,
            from_path: "n_root".into(),
            to_path: None,
            coordinate: None,
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemMoveReverted");
    }

    #[test]
    fn item_deleted_serde_roundtrip() {
        let evt = DomainEvent::ItemDeleted(ItemDeletedData {
            reason: Some("broken".into()),
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemDeleted");
    }

    #[test]
    fn item_restored_serde_roundtrip() {
        let evt = DomainEvent::ItemRestored(ItemRestoredData {
            from_event_id: None,
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemRestored");
    }

    #[test]
    fn item_image_added_serde_roundtrip() {
        let evt = DomainEvent::ItemImageAdded(ItemImageAddedData {
            path: "/img/a.jpg".into(),
            caption: Some("front".into()),
            order: 0,
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemImageAdded");
    }

    #[test]
    fn item_image_removed_serde_roundtrip() {
        let evt = DomainEvent::ItemImageRemoved(ItemImageRemovedData {
            path: "/img/a.jpg".into(),
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemImageRemoved");
    }

    #[test]
    fn ext_code_added_serde_roundtrip() {
        let evt = DomainEvent::ItemExternalCodeAdded(ExternalCodeData {
            code_type: "UPC".into(),
            value: "012345678901".into(),
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemExternalCodeAdded");
    }

    #[test]
    fn ext_code_removed_serde_roundtrip() {
        let evt = DomainEvent::ItemExternalCodeRemoved(ExternalCodeData {
            code_type: "UPC".into(),
            value: "012345678901".into(),
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemExternalCodeRemoved");
    }

    #[test]
    fn quantity_adjusted_serde_roundtrip() {
        let evt = DomainEvent::ItemQuantityAdjusted(QuantityAdjustedData {
            old_qty: Some(5),
            new_qty: 10,
            reason: None,
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemQuantityAdjusted");
    }

    #[test]
    fn container_schema_updated_serde_roundtrip() {
        let evt = DomainEvent::ContainerSchemaUpdated(ContainerSchemaUpdatedData {
            old_schema: None,
            new_schema: serde_json::json!({"type": "grid"}),
        });
        assert_eq!(roundtrip(&evt).event_type(), "ContainerSchemaUpdated");
    }

    #[test]
    fn barcode_generated_serde_roundtrip() {
        let evt = DomainEvent::BarcodeGenerated(BarcodeGeneratedData {
            barcode: "HOM-000100".into(),
            assigned_to: None,
        });
        assert_eq!(roundtrip(&evt).event_type(), "BarcodeGenerated");
    }

    #[test]
    fn event_type_names_are_distinct() {
        let types = vec![
            DomainEvent::ItemCreated(ItemCreatedData {
                system_barcode: String::new(), node_id: String::new(), name: None,
                description: None, category: None, tags: vec![], is_container: false,
                container_path: String::new(), parent_id: Uuid::nil(), coordinate: None,
                location_schema: None, max_capacity_cc: None, max_weight_grams: None,
                dimensions: None, weight_grams: None, is_fungible: false,
                fungible_quantity: None, fungible_unit: None, external_codes: vec![],
                condition: None, acquisition_date: None, acquisition_cost: None,
                current_value: None, depreciation_rate: None, warranty_expiry: None,
                metadata: serde_json::json!({}),
            }).event_type(),
            DomainEvent::ItemUpdated(ItemUpdatedData { changes: vec![] }).event_type(),
            DomainEvent::ItemMoved(ItemMovedData {
                from_container_id: None, to_container_id: Uuid::nil(),
                from_path: None, to_path: String::new(), coordinate: None,
            }).event_type(),
            DomainEvent::ItemDeleted(ItemDeletedData { reason: None }).event_type(),
            DomainEvent::ItemRestored(ItemRestoredData { from_event_id: None }).event_type(),
            DomainEvent::ItemImageAdded(ItemImageAddedData {
                path: String::new(), caption: None, order: 0,
            }).event_type(),
            DomainEvent::ItemImageRemoved(ItemImageRemovedData { path: String::new() }).event_type(),
            DomainEvent::ItemExternalCodeAdded(ExternalCodeData {
                code_type: String::new(), value: String::new(),
            }).event_type(),
            DomainEvent::ItemExternalCodeRemoved(ExternalCodeData {
                code_type: String::new(), value: String::new(),
            }).event_type(),
            DomainEvent::ItemQuantityAdjusted(QuantityAdjustedData {
                old_qty: None, new_qty: 0, reason: None,
            }).event_type(),
            DomainEvent::ContainerSchemaUpdated(ContainerSchemaUpdatedData {
                old_schema: None, new_schema: serde_json::json!(null),
            }).event_type(),
            DomainEvent::BarcodeGenerated(BarcodeGeneratedData {
                barcode: String::new(), assigned_to: None,
            }).event_type(),
        ];
        let set: std::collections::HashSet<&str> = types.iter().copied().collect();
        assert_eq!(set.len(), types.len(), "event_type() names must be unique");
    }

    #[test]
    fn event_metadata_default_has_all_none() {
        let m = EventMetadata::default();
        assert!(m.correlation_id.is_none());
        assert!(m.causation_id.is_none());
        assert!(m.session_id.is_none());
        assert!(m.batch_id.is_none());
    }
}
