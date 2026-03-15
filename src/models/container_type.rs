use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A named template for containers, pre-populating physical dimensions and limits.
/// Typically created inline when making a new container from the UX.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContainerType {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub default_max_capacity_cc: Option<rust_decimal::Decimal>,
    pub default_max_weight_grams: Option<rust_decimal::Decimal>,
    /// Default dimensions JSON: `{width_cm, height_cm, depth_cm}`.
    pub default_dimensions: Option<serde_json::Value>,
    /// Default internal coordinate system template.
    pub default_location_schema: Option<serde_json::Value>,
    /// Optional UI icon identifier (e.g. "bin", "shelf", "drawer").
    pub icon: Option<String>,
    /// Semantic designation: outbox, storage, transit, workspace, etc. Free-text, not an enum.
    pub purpose: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new container type.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateContainerTypeRequest {
    pub name: String,
    pub description: Option<String>,
    pub default_max_capacity_cc: Option<f64>,
    pub default_max_weight_grams: Option<f64>,
    pub default_dimensions: Option<serde_json::Value>,
    pub default_location_schema: Option<serde_json::Value>,
    pub icon: Option<String>,
    /// Semantic designation for this container type (e.g. "outbox", "storage").
    pub purpose: Option<String>,
}

/// Request to partially update a container type.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateContainerTypeRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub default_max_capacity_cc: Option<f64>,
    pub default_max_weight_grams: Option<f64>,
    pub default_dimensions: Option<serde_json::Value>,
    pub default_location_schema: Option<serde_json::Value>,
    pub icon: Option<String>,
    /// Semantic designation for this container type (e.g. "outbox", "storage").
    pub purpose: Option<String>,
}
