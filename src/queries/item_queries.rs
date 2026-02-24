use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::event::StoredEvent;
use crate::models::item::{AncestorEntry, Item, ItemDetail};

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

        let ancestors = self.resolve_ancestors(&item.container_path).await?;

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

        let ancestors = self.resolve_ancestors(&item.container_path).await?;

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
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number
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

    /// Resolve ancestor breadcrumbs from an LTREE path string.
    /// Uses a single batch query instead of N+1 per-label queries.
    async fn resolve_ancestors(
        &self,
        path: &Option<String>,
    ) -> AppResult<Vec<AncestorEntry>> {
        let path_str = match path {
            Some(p) => p,
            None => return Ok(vec![]),
        };

        let labels: Vec<&str> = path_str.split('.').collect();
        let labels_owned: Vec<String> = labels.iter().map(|s| s.to_string()).collect();

        // Single batch query for all ancestor node_ids
        let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, String)>(
            "SELECT id, system_barcode, name, node_id FROM items WHERE node_id = ANY($1)",
        )
        .bind(&labels_owned)
        .fetch_all(&self.pool)
        .await?;

        // Build lookup map and reorder by path position
        let lookup: std::collections::HashMap<&str, &(Uuid, String, Option<String>, String)> =
            rows.iter().map(|r| (r.3.as_str(), r)).collect();

        let mut ancestors = Vec::with_capacity(labels.len());
        for (depth, label) in labels.iter().enumerate() {
            if let Some((id, barcode, name, node_id)) = lookup.get(label) {
                ancestors.push(AncestorEntry {
                    id: *id,
                    system_barcode: barcode.clone(),
                    name: name.clone(),
                    node_id: node_id.clone(),
                    depth,
                });
            }
        }

        Ok(ancestors)
    }
}
