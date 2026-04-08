use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// B5: Deserialize a Decimal from either a JSON string OR a JSON number.
/// Events written before the precision fix stored these as f64 numbers; new events store strings.
mod decimal_compat {
    use rust_decimal::Decimal;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize_opt<'de, D>(d: D) -> Result<Option<Decimal>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Option<serde_json::Value> = Option::deserialize(d)?;
        match v {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(s)) => s.parse::<Decimal>().map(Some).map_err(serde::de::Error::custom),
            Some(serde_json::Value::Number(n)) => {
                // Legacy events stored these as floats — round-trip via text.
                n.to_string()
                    .parse::<Decimal>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
            _ => Err(serde::de::Error::custom("expected decimal as string or number")),
        }
    }
}

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
    pub schema_version: i32,
}

/// Domain event variants for type-safe event handling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    ItemCreated(Box<ItemCreatedData>),
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
    /// Assigns (or reassigns) a system barcode to an existing item.
    /// The previous barcode is stored for undo support.
    ItemBarcodeAssigned(ItemBarcodeAssignedData),
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
            DomainEvent::ItemBarcodeAssigned(_) => "ItemBarcodeAssigned",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCreatedData {
    /// System barcode assigned to the item at creation time.
    /// May be None if the item was created without a barcode (opt-in assignment later).
    /// Old events written before migration 0012 always have a value; deserialization
    /// via `#[serde(default)]` handles forward/backward compatibility.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub system_barcode: Option<String>,
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
    /// Absent in events written before this field was added.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub currency: Option<String>,
    pub acquisition_date: Option<String>,
    // B5: stored as string in new events; legacy events stored as float (handled by decimal_compat).
    #[serde(default, deserialize_with = "decimal_compat::deserialize_opt")]
    pub acquisition_cost: Option<rust_decimal::Decimal>,
    #[serde(default, deserialize_with = "decimal_compat::deserialize_opt")]
    pub current_value: Option<rust_decimal::Decimal>,
    #[serde(default, deserialize_with = "decimal_compat::deserialize_opt")]
    pub depreciation_rate: Option<rust_decimal::Decimal>,
    pub warranty_expiry: Option<String>,
    pub metadata: serde_json::Value,
    /// DI-2: Original creation timestamp preserved for accurate projection rebuild.
    /// Absent in events written before this field was added; defaults to None.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub created_at: Option<DateTime<Utc>>,
    /// Container type FK (only meaningful when is_container = true).
    /// Absent in events written before this field was added.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub container_type_id: Option<Uuid>,
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
    /// The item's coordinate at its source location, captured at move time.
    /// Used by undo to restore the original placement. Absent in legacy events.
    #[serde(default)]
    pub from_coordinate: Option<serde_json::Value>,
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
    /// Preserved from the original image for undo restoration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub caption: Option<String>,
    /// Preserved from the original image for undo restoration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub order: Option<i32>,
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
    /// Maps old label → new label for coordinate renames on children.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub label_renames: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarcodeGeneratedData {
    pub barcode: String,
    pub assigned_to: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemBarcodeAssignedData {
    /// The new barcode value being assigned.
    pub barcode: String,
    /// Previous barcode (None if the item had no barcode before).
    /// Stored so the assignment can be undone.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub previous_barcode: Option<String>,
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
    /// Client-side scan timestamp (RFC 3339). Preserved from StockerBatchEvent.scanned_at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scanned_at: Option<String>,
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
        let evt = DomainEvent::ItemCreated(Box::new(ItemCreatedData {
            system_barcode: Some("HOM-000001".into()),
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
            currency: None,
            acquisition_date: None,
            acquisition_cost: None,
            current_value: None,
            depreciation_rate: None,
            warranty_expiry: None,
            metadata: serde_json::json!({}),
            created_at: None,
            container_type_id: None,
        }));
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
            from_coordinate: None,
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
        let evt = DomainEvent::ItemRestored(ItemRestoredData { from_event_id: None });
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
            caption: None,
            order: None,
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
            label_renames: std::collections::HashMap::new(),
        });
        assert_eq!(roundtrip(&evt).event_type(), "ContainerSchemaUpdated");
    }

    #[test]
    fn container_schema_updated_with_label_renames_roundtrip() {
        let mut renames = std::collections::HashMap::new();
        renames.insert("top shelf".to_string(), "upper shelf".to_string());
        renames.insert("bottom shelf".to_string(), "lower shelf".to_string());
        let evt = DomainEvent::ContainerSchemaUpdated(ContainerSchemaUpdatedData {
            old_schema: Some(serde_json::json!({"type":"abstract","labels":["top shelf","bottom shelf"]})),
            new_schema: serde_json::json!({"type":"abstract","labels":["upper shelf","lower shelf"]}),
            label_renames: renames.clone(),
        });
        let rt = roundtrip(&evt);
        if let DomainEvent::ContainerSchemaUpdated(d) = rt {
            assert_eq!(
                d.label_renames.get("top shelf").map(|s| s.as_str()),
                Some("upper shelf")
            );
            assert_eq!(
                d.label_renames.get("bottom shelf").map(|s| s.as_str()),
                Some("lower shelf")
            );
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn empty_label_renames_not_serialised() {
        // label_renames should be omitted from JSON when empty (skip_serializing_if)
        let evt = DomainEvent::ContainerSchemaUpdated(ContainerSchemaUpdatedData {
            old_schema: None,
            new_schema: serde_json::json!({"type": "geo"}),
            label_renames: std::collections::HashMap::new(),
        });
        let json = serde_json::to_value(&evt).unwrap();
        // The inner data object must NOT contain label_renames key when empty
        if let serde_json::Value::Object(map) = &json {
            let inner = map.iter().find_map(|(_, v)| v.as_object());
            if let Some(inner_map) = inner {
                assert!(
                    !inner_map.contains_key("label_renames"),
                    "empty label_renames should be omitted"
                );
            }
        }
    }

    #[test]
    fn item_created_with_currency_roundtrip() {
        let evt = DomainEvent::ItemCreated(Box::new(ItemCreatedData {
            system_barcode: None,
            node_id: "n_test".into(),
            name: Some("Expensive Widget".into()),
            description: None,
            category: None,
            tags: vec![],
            is_container: false,
            container_path: "n_root".into(),
            parent_id: Uuid::nil(),
            coordinate: None,
            location_schema: None,
            max_capacity_cc: None,
            max_weight_grams: None,
            dimensions: None,
            weight_grams: None,
            is_fungible: false,
            fungible_quantity: None,
            fungible_unit: None,
            external_codes: vec![],
            condition: None,
            currency: Some("EUR".into()),
            acquisition_date: None,
            acquisition_cost: None,
            current_value: None,
            depreciation_rate: None,
            warranty_expiry: None,
            metadata: serde_json::json!({}),
            created_at: None,
            container_type_id: None,
        }));
        let rt = roundtrip(&evt);
        if let DomainEvent::ItemCreated(d) = rt {
            assert_eq!(d.currency.as_deref(), Some("EUR"));
        } else {
            panic!("wrong variant");
        }
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
    fn item_barcode_assigned_serde_roundtrip() {
        let evt = DomainEvent::ItemBarcodeAssigned(ItemBarcodeAssignedData {
            barcode: "HOM-000099".into(),
            previous_barcode: None,
        });
        assert_eq!(roundtrip(&evt).event_type(), "ItemBarcodeAssigned");
    }

    #[test]
    fn item_created_without_barcode_roundtrip() {
        // Ensure system_barcode: None serialises correctly and is backward-compat
        let evt = DomainEvent::ItemCreated(Box::new(ItemCreatedData {
            system_barcode: None,
            node_id: "n_aabbccdd0011".into(),
            name: None,
            description: None,
            category: None,
            tags: vec![],
            is_container: false,
            container_path: "n_root".into(),
            parent_id: Uuid::nil(),
            coordinate: None,
            location_schema: None,
            max_capacity_cc: None,
            max_weight_grams: None,
            dimensions: None,
            weight_grams: None,
            is_fungible: false,
            fungible_quantity: None,
            fungible_unit: None,
            external_codes: vec![],
            condition: None,
            currency: None,
            acquisition_date: None,
            acquisition_cost: None,
            current_value: None,
            depreciation_rate: None,
            warranty_expiry: None,
            metadata: serde_json::json!({}),
            created_at: None,
            container_type_id: None,
        }));
        let rt = roundtrip(&evt);
        if let DomainEvent::ItemCreated(d) = rt {
            assert!(d.system_barcode.is_none());
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn event_type_names_are_distinct() {
        let types = vec![
            DomainEvent::ItemCreated(Box::new(ItemCreatedData {
                system_barcode: None,
                node_id: String::new(),
                name: None,
                description: None,
                category: None,
                tags: vec![],
                is_container: false,
                container_path: String::new(),
                parent_id: Uuid::nil(),
                coordinate: None,
                location_schema: None,
                max_capacity_cc: None,
                max_weight_grams: None,
                dimensions: None,
                weight_grams: None,
                is_fungible: false,
                fungible_quantity: None,
                fungible_unit: None,
                external_codes: vec![],
                condition: None,
                currency: None,
                acquisition_date: None,
                acquisition_cost: None,
                current_value: None,
                depreciation_rate: None,
                warranty_expiry: None,
                metadata: serde_json::json!({}),
                created_at: None,
                container_type_id: None,
            }))
            .event_type(),
            DomainEvent::ItemUpdated(ItemUpdatedData { changes: vec![] }).event_type(),
            DomainEvent::ItemMoved(ItemMovedData {
                from_container_id: None,
                to_container_id: Uuid::nil(),
                from_path: None,
                to_path: String::new(),
                coordinate: None,
                from_coordinate: None,
            })
            .event_type(),
            DomainEvent::ItemDeleted(ItemDeletedData { reason: None }).event_type(),
            DomainEvent::ItemRestored(ItemRestoredData { from_event_id: None }).event_type(),
            DomainEvent::ItemImageAdded(ItemImageAddedData {
                path: String::new(),
                caption: None,
                order: 0,
            })
            .event_type(),
            DomainEvent::ItemImageRemoved(ItemImageRemovedData {
                path: String::new(),
                caption: None,
                order: None,
            })
            .event_type(),
            DomainEvent::ItemExternalCodeAdded(ExternalCodeData {
                code_type: String::new(),
                value: String::new(),
            })
            .event_type(),
            DomainEvent::ItemExternalCodeRemoved(ExternalCodeData {
                code_type: String::new(),
                value: String::new(),
            })
            .event_type(),
            DomainEvent::ItemQuantityAdjusted(QuantityAdjustedData {
                old_qty: None,
                new_qty: 0,
                reason: None,
            })
            .event_type(),
            DomainEvent::ContainerSchemaUpdated(ContainerSchemaUpdatedData {
                old_schema: None,
                new_schema: serde_json::json!(null),
                label_renames: std::collections::HashMap::new(),
            })
            .event_type(),
            DomainEvent::BarcodeGenerated(BarcodeGeneratedData {
                barcode: String::new(),
                assigned_to: None,
            })
            .event_type(),
            DomainEvent::ItemBarcodeAssigned(ItemBarcodeAssignedData {
                barcode: String::new(),
                previous_barcode: None,
            })
            .event_type(),
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
