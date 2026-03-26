use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A managed tag that can be applied to items.
/// Renaming a tag here instantly renames it on all tagged items.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    /// Number of items currently using this tag (populated by specific queries).
    #[sqlx(default)]
    pub item_count: Option<i64>,
}

/// A managed category that items belong to.
/// Optional parent links allow a shallow category hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub parent_category_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Number of items currently in this category (populated by specific queries).
    #[sqlx(default)]
    pub item_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameTagRequest {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    pub description: Option<String>,
    pub parent_category_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCategoryRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Absent → don't change; `null` → clear to top-level; UUID → set new parent.
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub parent_category_id: Option<Option<Uuid>>,
}

/// Deserialize a field that distinguishes absent (None) from explicit null (Some(None)).
fn deserialize_optional_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}
