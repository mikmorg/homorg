use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::event::StoredEvent;
use crate::models::item::{ImageEntry, Item, ItemDetail};
use crate::queries::common::resolve_ancestors;

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
        let item = sqlx::query_as::<_, Item>(
            "SELECT * FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {id} not found")))?;

        let ancestors = resolve_ancestors(&self.pool, &item.container_path).await?;

        Ok(ItemDetail { item, ancestors })
    }

    /// Get full item detail by system barcode.
    pub async fn get_by_barcode(&self, barcode: &str) -> AppResult<ItemDetail> {
        let item = sqlx::query_as::<_, Item>(
            "SELECT * FROM items WHERE system_barcode = $1 AND is_deleted = FALSE",
        )
        .bind(barcode)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item with barcode {barcode} not found")))?;

        let ancestors = resolve_ancestors(&self.pool, &item.container_path).await?;

        Ok(ItemDetail { item, ancestors })
    }

    /// Get paginated event history for an item.
    pub async fn get_history(
        &self,
        item_id: Uuid,
        after_seq: Option<i64>,
        limit: i64,
    ) -> AppResult<Vec<StoredEvent>> {
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
        let images_json: serde_json::Value = sqlx::query_scalar(
            "SELECT images FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        let images: Vec<ImageEntry> = serde_json::from_value(images_json)
            .map_err(|_| AppError::Internal("Failed to parse images".into()))?;

        Ok(images)
    }
}
