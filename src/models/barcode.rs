use serde::{Deserialize, Serialize};

/// Result of resolving a scanned barcode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BarcodeResolution {
    #[serde(rename = "system")]
    System {
        barcode: String,
        item_id: uuid::Uuid,
    },
    #[serde(rename = "external")]
    External {
        code_type: String,
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        item_id: Option<uuid::Uuid>,
    },
    #[serde(rename = "unknown_system")]
    UnknownSystem {
        barcode: String,
    },
    #[serde(rename = "unknown")]
    Unknown {
        value: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedBarcode {
    pub barcode: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateBatchRequest {
    pub count: u32,
}
