use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Result of resolving a scanned barcode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BarcodeResolution {
    /// A homorg system barcode (e.g. "HOM-000042") that maps to a known item.
    #[serde(rename = "system")]
    System { barcode: String, item_id: Uuid },
    /// A commercial barcode (UPC, EAN, ISBN …). May match zero or multiple items
    /// (e.g. the same ISBN owned multiple times, or no item registered yet).
    #[serde(rename = "external")]
    External {
        code_type: String,
        value: String,
        /// All items that carry this external code.
        /// Empty when no item references this barcode.
        /// More than one entry means the caller must disambiguate.
        item_ids: Vec<Uuid>,
    },
    /// A system-format barcode that exists in the sequence but is not yet
    /// assigned to any item (pre-printed label, unregistered).
    #[serde(rename = "unknown_system")]
    UnknownSystem { barcode: String },
    /// Barcode does not match any known format or item.
    #[serde(rename = "unknown")]
    Unknown { value: String },
    /// A system barcode that has been pre-assigned as a container or item preset.
    /// Scanning this in a stocker session auto-creates the record without a name prompt.
    #[serde(rename = "preset")]
    Preset {
        barcode: String,
        is_container: bool,
        container_type_id: Option<Uuid>,
        container_type_name: Option<String>,
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

/// Request to assign (or reassign) a system barcode to an item.
#[derive(Debug, Clone, Deserialize)]
pub struct AssignBarcodeRequest {
    /// The barcode string to assign.  Must be unique across all items.
    /// Can be a pre-generated system barcode (e.g. "HOM-000042") or any
    /// custom string that fits within 32 characters.
    pub barcode: String,
}
