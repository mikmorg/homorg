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
    pub active_item_id: Option<Uuid>,
    pub photo_needed: bool,
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
    Resolve { barcode: String, scanned_at: DateTime<Utc> },
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

#[cfg(test)]
mod tests {
    use super::*;

    const TS: &str = "2024-01-15T10:00:00Z";
    const ID1: &str = "00000000-0000-0000-0000-000000000001";
    const ID2: &str = "00000000-0000-0000-0000-000000000002";

    // ── StockerBatchEvent deserialization ─────────────────────────────────────

    #[test]
    fn deserialize_set_context() {
        let json = format!(r#"{{"type":"set_context","container_id":"{ID1}","scanned_at":"{TS}"}}"#);
        let ev: StockerBatchEvent = serde_json::from_str(&json).unwrap();
        match ev {
            StockerBatchEvent::SetContext { container_id, .. } => {
                assert_eq!(container_id.to_string(), ID1);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn deserialize_move_item_with_coordinate() {
        let json = format!(
            r#"{{"type":"move_item","item_id":"{ID1}","coordinate":{{"type":"abstract","value":"A"}},"scanned_at":"{TS}"}}"#
        );
        let ev: StockerBatchEvent = serde_json::from_str(&json).unwrap();
        match ev {
            StockerBatchEvent::MoveItem {
                item_id, coordinate, ..
            } => {
                assert_eq!(item_id.to_string(), ID1);
                assert!(coordinate.is_some());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn deserialize_move_item_without_coordinate() {
        let json = format!(r#"{{"type":"move_item","item_id":"{ID1}","scanned_at":"{TS}"}}"#);
        let ev: StockerBatchEvent = serde_json::from_str(&json).unwrap();
        match ev {
            StockerBatchEvent::MoveItem { coordinate, .. } => assert!(coordinate.is_none()),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn deserialize_create_and_place_minimal() {
        let json = format!(r#"{{"type":"create_and_place","barcode":"HOM-000001","scanned_at":"{TS}"}}"#);
        let ev: StockerBatchEvent = serde_json::from_str(&json).unwrap();
        match ev {
            StockerBatchEvent::CreateAndPlace {
                barcode,
                name,
                is_container,
                ..
            } => {
                assert_eq!(barcode, "HOM-000001");
                assert!(name.is_none());
                assert!(is_container.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn deserialize_create_and_place_full() {
        let json = format!(
            r#"{{"type":"create_and_place","barcode":"HOM-000002","name":"Widget","description":"A thing",
               "category":"Electronics","tags":["new","sale"],"is_container":false,
               "condition":"good","is_fungible":true,"fungible_quantity":10,
               "fungible_unit":"pcs","scanned_at":"{TS}"}}"#
        );
        let ev: StockerBatchEvent = serde_json::from_str(&json).unwrap();
        match ev {
            StockerBatchEvent::CreateAndPlace {
                name,
                tags,
                is_fungible,
                fungible_quantity,
                ..
            } => {
                assert_eq!(name.as_deref(), Some("Widget"));
                assert_eq!(tags.as_deref(), Some(["new".to_string(), "sale".to_string()].as_ref()));
                assert_eq!(is_fungible, Some(true));
                assert_eq!(fungible_quantity, Some(10));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn deserialize_resolve() {
        let json = format!(r#"{{"type":"resolve","barcode":"HOM-000042","scanned_at":"{TS}"}}"#);
        let ev: StockerBatchEvent = serde_json::from_str(&json).unwrap();
        match ev {
            StockerBatchEvent::Resolve { barcode, .. } => assert_eq!(barcode, "HOM-000042"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn deserialize_unknown_type_fails() {
        let json = format!(r#"{{"type":"unknown_event","scanned_at":"{TS}"}}"#);
        assert!(serde_json::from_str::<StockerBatchEvent>(&json).is_err());
    }

    #[test]
    fn deserialize_set_context_missing_container_id_fails() {
        let json = format!(r#"{{"type":"set_context","scanned_at":"{TS}"}}"#);
        assert!(serde_json::from_str::<StockerBatchEvent>(&json).is_err());
    }

    // ── StockerBatchResult serialization ─────────────────────────────────────

    #[test]
    fn serialize_context_set() {
        let id = Uuid::parse_str(ID1).unwrap();
        let r = StockerBatchResult::ContextSet {
            index: 0,
            status: "ok".into(),
            container_id: id,
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["type"], "context_set");
        assert_eq!(v["container_id"].as_str().unwrap(), ID1);
        assert_eq!(v["index"], 0);
    }

    #[test]
    fn serialize_moved() {
        let id = Uuid::parse_str(ID1).unwrap();
        let r = StockerBatchResult::Moved {
            index: 1,
            status: "ok".into(),
            event_id: id,
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["type"], "moved");
        assert_eq!(v["event_id"].as_str().unwrap(), ID1);
    }

    #[test]
    fn serialize_created() {
        let ev_id = Uuid::parse_str(ID1).unwrap();
        let item_id = Uuid::parse_str(ID2).unwrap();
        let r = StockerBatchResult::Created {
            index: 2,
            status: "ok".into(),
            event_id: ev_id,
            item_id,
            needs_details: true,
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["type"], "created");
        assert_eq!(v["needs_details"], true);
        assert_eq!(v["item_id"].as_str().unwrap(), ID2);
    }

    // ── StartSessionRequest deserialization ──────────────────────────────────

    #[test]
    fn deserialize_start_session_empty() {
        let r: StartSessionRequest = serde_json::from_str("{}").unwrap();
        assert!(r.device_id.is_none());
        assert!(r.notes.is_none());
        assert!(r.initial_container_id.is_none());
    }

    #[test]
    fn deserialize_start_session_with_container() {
        let json = format!(r#"{{"initial_container_id":"{ID1}","device_id":"scanner-1"}}"#);
        let r: StartSessionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(r.initial_container_id.unwrap().to_string(), ID1);
        assert_eq!(r.device_id.as_deref(), Some("scanner-1"));
    }

    // ── StockerBatchRequest round-trip ────────────────────────────────────────

    #[test]
    fn deserialize_batch_request_mixed_events() {
        let json = format!(
            r#"{{"events":[
                {{"type":"set_context","container_id":"{ID1}","scanned_at":"{TS}"}},
                {{"type":"move_item","item_id":"{ID2}","scanned_at":"{TS}"}},
                {{"type":"resolve","barcode":"EAN-1234567890","scanned_at":"{TS}"}}
            ]}}"#
        );
        let req: StockerBatchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.events.len(), 3);
        assert!(matches!(req.events[0], StockerBatchEvent::SetContext { .. }));
        assert!(matches!(req.events[1], StockerBatchEvent::MoveItem { .. }));
        assert!(matches!(req.events[2], StockerBatchEvent::Resolve { .. }));
    }
}
