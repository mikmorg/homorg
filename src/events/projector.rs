use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::event::*;

/// Synchronous projector: applies domain events to the items read projection table.
/// All projection updates run within the same transaction as the event append.
pub struct Projector;

impl Projector {
    /// Apply a domain event to the items read projection.
    /// Must be called inside the same transaction as the event store append.
    pub async fn apply(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        aggregate_id: Uuid,
        event: &DomainEvent,
        actor_id: Uuid,
    ) -> AppResult<()> {
        match event {
            DomainEvent::ItemCreated(data) => Self::project_item_created(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ItemUpdated(data) => Self::project_item_updated(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ItemMoved(data) => Self::project_item_moved(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ItemMoveReverted(data) => Self::project_item_move_reverted(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ItemDeleted(_) => Self::project_item_deleted(tx, aggregate_id, actor_id).await,
            DomainEvent::ItemRestored(_) => Self::project_item_restored(tx, aggregate_id, actor_id).await,
            DomainEvent::ItemImageAdded(data) => Self::project_image_added(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ItemImageRemoved(data) => Self::project_image_removed(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ItemExternalCodeAdded(data) => Self::project_ext_code_added(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ItemExternalCodeRemoved(data) => Self::project_ext_code_removed(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ItemQuantityAdjusted(data) => Self::project_quantity_adjusted(tx, aggregate_id, data, actor_id).await,
            DomainEvent::ContainerSchemaUpdated(data) => Self::project_schema_updated(tx, aggregate_id, data, actor_id).await,
            DomainEvent::BarcodeGenerated(_) => Ok(()), // No projection change
        }
    }

    async fn project_item_created(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ItemCreatedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        let ext_codes = serde_json::to_value(&data.external_codes)
            .unwrap_or_else(|_| serde_json::json!([]));

        sqlx::query(
            r#"
            INSERT INTO items (
                id, system_barcode, ltree_label,
                name, description, category, tags,
                is_container, container_path, parent_id, coordinate,
                location_schema, max_capacity_cc, max_weight_grams,
                dimensions, weight_grams,
                is_fungible, fungible_quantity, fungible_unit,
                external_codes,
                condition, acquisition_date, acquisition_cost, current_value, depreciation_rate, warranty_expiry,
                metadata,
                created_by, updated_by
            ) VALUES (
                $1, $2, $3,
                $4, $5, $6, $7,
                $8, $9::ltree, $10, $11,
                $12, $13, $14,
                $15, $16,
                $17, $18, $19,
                $20,
                $21, $22::date, $23, $24, $25, $26::date,
                $27,
                $28, $28
            )
            "#,
        )
        .bind(id)
        .bind(&data.system_barcode)
        .bind(&data.ltree_label)
        .bind(&data.name)
        .bind(&data.description)
        .bind(&data.category)
        .bind(&data.tags)
        .bind(data.is_container)
        .bind(&data.container_path)
        .bind(data.parent_id)
        .bind(&data.coordinate)
        .bind(&data.location_schema)
        .bind(data.max_capacity_cc)
        .bind(data.max_weight_grams)
        .bind(&data.dimensions)
        .bind(data.weight_grams)
        .bind(data.is_fungible)
        .bind(data.fungible_quantity)
        .bind(&data.fungible_unit)
        .bind(&ext_codes)
        .bind(&data.condition)
        .bind(&data.acquisition_date)
        .bind(data.acquisition_cost)
        .bind(data.current_value)
        .bind(data.depreciation_rate)
        .bind(&data.warranty_expiry)
        .bind(&data.metadata)
        .bind(actor_id)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    async fn project_item_updated(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ItemUpdatedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        // Apply each field change individually via dynamic SQL.
        // We use a safe allowlist of fields to prevent injection.
        let allowed_text_fields = [
            "name", "description", "category", "condition", "fungible_unit",
        ];
        let allowed_numeric_fields = [
            "max_capacity_cc", "max_weight_grams", "weight_grams",
            "acquisition_cost", "current_value", "depreciation_rate",
        ];
        let allowed_jsonb_fields = [
            "coordinate", "location_schema", "dimensions", "metadata",
        ];

        for change in &data.changes {
            let field = change.field.as_str();

            if allowed_text_fields.contains(&field) {
                let value = change.new.as_str().map(|s| s.to_string());
                let query = format!("UPDATE items SET {field} = $1, updated_by = $2 WHERE id = $3");
                sqlx::query(&query)
                    .bind(&value)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            } else if allowed_numeric_fields.contains(&field) {
                let value = change.new.as_f64();
                let query = format!("UPDATE items SET {field} = $1, updated_by = $2 WHERE id = $3");
                sqlx::query(&query)
                    .bind(value)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            } else if allowed_jsonb_fields.contains(&field) {
                let query = format!("UPDATE items SET {field} = $1, updated_by = $2 WHERE id = $3");
                sqlx::query(&query)
                    .bind(&change.new)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            } else if field == "tags" {
                let tags: Vec<String> = serde_json::from_value(change.new.clone()).unwrap_or_default();
                sqlx::query("UPDATE items SET tags = $1, updated_by = $2 WHERE id = $3")
                    .bind(&tags)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            } else if field == "is_container" || field == "is_fungible" {
                let value = change.new.as_bool().unwrap_or(false);
                let query = format!("UPDATE items SET {field} = $1, updated_by = $2 WHERE id = $3");
                sqlx::query(&query)
                    .bind(value)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            } else if field == "fungible_quantity" {
                let value = change.new.as_i64().map(|v| v as i32);
                sqlx::query("UPDATE items SET fungible_quantity = $1, updated_by = $2 WHERE id = $3")
                    .bind(value)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            } else if field == "acquisition_date" || field == "warranty_expiry" {
                let value = change.new.as_str().map(|s| s.to_string());
                let query = format!("UPDATE items SET {field} = $1::date, updated_by = $2 WHERE id = $3");
                sqlx::query(&query)
                    .bind(&value)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            } else {
                tracing::warn!(field, "Ignoring unknown field in ItemUpdated projection");
            }
        }

        Ok(())
    }

    async fn project_item_moved(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ItemMovedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        // Get the old path for cascading
        let old_path: Option<String> = sqlx::query_scalar(
            "SELECT container_path::text FROM items WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;

        // Update the moved item itself
        sqlx::query(
            r#"
            UPDATE items
            SET parent_id = $1, container_path = $2::ltree, coordinate = $3, updated_by = $4
            WHERE id = $5
            "#,
        )
        .bind(data.to_container_id)
        .bind(&data.to_path)
        .bind(&data.coordinate)
        .bind(actor_id)
        .bind(id)
        .execute(&mut **tx)
        .await?;

        // Cascade path update to all descendants
        if let Some(old_p) = &old_path {
            let new_prefix = &data.to_path;
            // Update descendants: replace old_path prefix with new_path prefix
            sqlx::query(
                r#"
                UPDATE items
                SET container_path = ($1::ltree || subpath(container_path, nlevel($2::ltree))),
                    updated_by = $3
                WHERE container_path <@ $2::ltree AND id != $4
                "#,
            )
            .bind(new_prefix)
            .bind(old_p)
            .bind(actor_id)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    async fn project_item_move_reverted(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ItemMoveRevertedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        // Reuse move logic: move back to original container
        let move_data = ItemMovedData {
            from_container_id: Some(data.from_container_id),
            to_container_id: data.to_container_id.unwrap_or(data.from_container_id),
            from_path: Some(data.from_path.clone()),
            to_path: data.to_path.clone().unwrap_or_else(|| data.from_path.clone()),
            coordinate: data.coordinate.clone(),
        };
        Self::project_item_moved(tx, id, &move_data, actor_id).await
    }

    async fn project_item_deleted(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        actor_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("UPDATE items SET is_deleted = TRUE, updated_by = $1 WHERE id = $2")
            .bind(actor_id)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    async fn project_item_restored(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        actor_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("UPDATE items SET is_deleted = FALSE, updated_by = $1 WHERE id = $2")
            .bind(actor_id)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    async fn project_image_added(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ItemImageAddedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        let entry = serde_json::json!({
            "path": data.path,
            "caption": data.caption,
            "order": data.order,
        });
        sqlx::query(
            "UPDATE items SET images = images || $1::jsonb, updated_by = $2 WHERE id = $3",
        )
        .bind(&entry)
        .bind(actor_id)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn project_image_removed(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ItemImageRemovedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        // Remove the image entry matching the path
        sqlx::query(
            r#"
            UPDATE items
            SET images = (
                SELECT COALESCE(jsonb_agg(elem), '[]'::jsonb)
                FROM jsonb_array_elements(images) AS elem
                WHERE elem->>'path' != $1
            ), updated_by = $2
            WHERE id = $3
            "#,
        )
        .bind(&data.path)
        .bind(actor_id)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn project_ext_code_added(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ExternalCodeData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        let entry = serde_json::json!({"type": data.code_type, "value": data.value});
        sqlx::query(
            "UPDATE items SET external_codes = external_codes || $1::jsonb, updated_by = $2 WHERE id = $3",
        )
        .bind(&entry)
        .bind(actor_id)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn project_ext_code_removed(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ExternalCodeData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE items
            SET external_codes = (
                SELECT COALESCE(jsonb_agg(elem), '[]'::jsonb)
                FROM jsonb_array_elements(external_codes) AS elem
                WHERE NOT (elem->>'type' = $1 AND elem->>'value' = $2)
            ), updated_by = $3
            WHERE id = $4
            "#,
        )
        .bind(&data.code_type)
        .bind(&data.value)
        .bind(actor_id)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn project_quantity_adjusted(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &QuantityAdjustedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("UPDATE items SET fungible_quantity = $1, updated_by = $2 WHERE id = $3")
            .bind(data.new_qty)
            .bind(actor_id)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    async fn project_schema_updated(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ContainerSchemaUpdatedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("UPDATE items SET location_schema = $1, updated_by = $2 WHERE id = $3")
            .bind(&data.new_schema)
            .bind(actor_id)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// Rebuild all projections by replaying the entire event store.
    /// WARNING: This truncates the items table (except seed data) and replays everything.
    pub async fn rebuild_all(pool: &PgPool) -> AppResult<u64> {
        let mut tx = pool.begin().await?;

        // Delete all non-seed items
        sqlx::query("DELETE FROM items WHERE id NOT IN ('00000000-0000-0000-0000-000000000001'::uuid, '00000000-0000-0000-0000-000000000002'::uuid)")
            .execute(&mut *tx)
            .await?;

        // Replay all events in order
        let events = sqlx::query_as::<_, StoredEvent>(
            r#"
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number
            FROM event_store
            ORDER BY id ASC
            "#,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count = events.len() as u64;
        for stored in &events {
            let domain_event: DomainEvent = serde_json::from_value(stored.event_data.clone())
                .map_err(|e| AppError::Internal(format!(
                    "Failed to deserialize event {}: {e}", stored.event_id
                )))?;

            let actor = stored.actor_id.unwrap_or(Uuid::nil());
            Self::apply(&mut tx, stored.aggregate_id, &domain_event, actor).await?;
        }

        tx.commit().await?;
        Ok(count)
    }
}
