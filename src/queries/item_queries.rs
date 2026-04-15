use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::event::StoredEvent;
use crate::models::item::{ImageEntry, Item, ItemDetail};
use crate::queries::common::resolve_ancestors;

// ── Base SELECT for full Item rows ──────────────────────────────────────────
// Joins categories, container_properties and fungible_properties so the flat
// Item struct keeps its existing field names without any `SELECT *`.
pub(crate) const ITEM_FULL_SELECT: &str = r#"
    i.id,
    i.system_barcode,
    i.node_id,
    i.name,
    i.description,
    cat.name                                                           AS category,
    i.category_id,
    COALESCE(ARRAY(
        SELECT t.name FROM item_tags it2
        JOIN tags t ON t.id = it2.tag_id
        WHERE it2.item_id = i.id
        ORDER BY t.name
    ), ARRAY[]::text[])                                                AS tags,
    i.is_container,
    i.container_path::text                                             AS container_path,
    i.parent_id,
    i.coordinate,
    cp.location_schema,
    cp.max_capacity_cc,
    cp.max_weight_grams,
    cp.container_type_id,
    i.dimensions,
    i.weight_grams,
    i.is_fungible,
    fp.quantity                                                        AS fungible_quantity,
    fp.unit                                                            AS fungible_unit,
    i.external_codes,
    i.condition,
    i.acquisition_date,
    i.acquisition_cost,
    i.current_value,
    i.depreciation_rate,
    i.warranty_expiry,
    i.metadata,
    i.images,
    i.is_deleted,
    i.deleted_at,
    i.created_at,
    i.updated_at,
    i.created_by,
    i.updated_by,
    i.currency,
    i.classification_confidence,
    i.needs_review,
    i.ai_description,
    i.ai_suggestions
FROM items i
LEFT JOIN categories        cat ON cat.id      = i.category_id
LEFT JOIN container_properties cp ON cp.item_id = i.id
LEFT JOIN fungible_properties  fp ON fp.item_id = i.id
"#;

/// Read-side query handler for items.
#[derive(Clone)]
pub struct ItemQueries {
    pool: PgPool,
}

impl ItemQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get full item detail by ID, including ancestor breadcrumbs.
    pub async fn get_by_id(&self, id: Uuid) -> AppResult<ItemDetail> {
        let sql = format!("SELECT {ITEM_FULL_SELECT} WHERE i.id = $1 AND i.is_deleted = FALSE");
        let item = sqlx::query_as::<_, Item>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item {id} not found")))?;

        let ancestors = resolve_ancestors(&self.pool, &item.container_path).await?;

        Ok(ItemDetail { item, ancestors })
    }

    /// Get full item detail by system barcode.
    pub async fn get_by_barcode(&self, barcode: &str) -> AppResult<ItemDetail> {
        let sql = format!("SELECT {ITEM_FULL_SELECT} WHERE i.system_barcode = $1 AND i.is_deleted = FALSE");
        let item = sqlx::query_as::<_, Item>(&sql)
            .bind(barcode)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item with barcode '{barcode}' not found")))?;

        let ancestors = resolve_ancestors(&self.pool, &item.container_path).await?;

        Ok(ItemDetail { item, ancestors })
    }

    /// Get paginated event history for an item.
    pub async fn get_history(&self, item_id: Uuid, after_seq: Option<i64>, limit: i64) -> AppResult<Vec<StoredEvent>> {
        let from = after_seq.unwrap_or(0);
        let rows = sqlx::query_as::<_, StoredEvent>(
            r#"
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
            FROM event_store
            WHERE aggregate_id = $1 AND sequence_number > $2
            ORDER BY sequence_number ASC
            LIMIT $3
            "#,
        )
        .bind(item_id)
        .bind(from)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get the images array for an item.
    pub async fn get_images(&self, item_id: Uuid) -> AppResult<Vec<ImageEntry>> {
        let images_json: serde_json::Value =
            sqlx::query_scalar("SELECT images FROM items WHERE id = $1 AND is_deleted = FALSE")
                .bind(item_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        // IQ-1: A NULL images column (items that have never had images) deserializes as
        // Value::Null via sqlx.  Calling from_value(Null) would fail; return empty Vec instead.
        let images: Vec<ImageEntry> = if images_json.is_null() {
            vec![]
        } else {
            serde_json::from_value(images_json).map_err(|_| AppError::Internal("Failed to parse images".into()))?
        };

        Ok(images)
    }
}
