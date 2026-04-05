use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use crate::constants::{ROOT_ID, USERS_ID};
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
            DomainEvent::ItemBarcodeAssigned(data) => Self::project_barcode_assigned(tx, aggregate_id, data, actor_id).await,
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

        // DI-2: Use the creation timestamp stored in the event so rebuild_all restores original
        // `created_at` values.  May be None for older events; fall back to NOW().
        let result = sqlx::query(
            r#"
            INSERT INTO items (
                id, system_barcode, node_id,
                name, description,
                is_container, container_path, parent_id, coordinate,
                dimensions, weight_grams,
                is_fungible,
                external_codes,
                condition, currency, acquisition_date, acquisition_cost, current_value,
                depreciation_rate, warranty_expiry,
                metadata,
                created_at,
                created_by, updated_by
            ) VALUES (
                $1, $2, $3,
                $4, $5,
                $6, $7::ltree, $8, $9,
                $10, $11,
                $12,
                $13,
                $14, $15, $16::date, $17, $18,
                $19, $20::date,
                $21,
                COALESCE($22::timestamptz, NOW()),
                $23, $23
            )
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(id)
        .bind(&data.system_barcode)
        .bind(&data.node_id)
        .bind(&data.name)
        .bind(&data.description)
        .bind(data.is_container)
        .bind(&data.container_path)
        .bind(data.parent_id)
        .bind(&data.coordinate)
        .bind(&data.dimensions)
        .bind(data.weight_grams)
        .bind(data.is_fungible)
        .bind(&ext_codes)
        .bind(&data.condition)
        .bind(&data.currency)
        .bind(&data.acquisition_date)
        .bind(data.acquisition_cost)
        .bind(data.current_value)
        .bind(data.depreciation_rate)
        .bind(&data.warranty_expiry)
        .bind(&data.metadata)
        .bind(data.created_at) // DI-2: bind original timestamp
        .bind(actor_id)
        .execute(&mut **tx)
        .await?;

        // ES-4: Log a warning if ON CONFLICT DO NOTHING silently swallowed a duplicate.
        if result.rows_affected() == 0 {
            tracing::warn!(item_id = %id, "project_item_created: ON CONFLICT DO NOTHING — duplicate ItemCreated event detected");
            // Row already exists; skip extension-table inserts to stay idempotent.
            return Ok(());
        }

        // ── Normalized category ──────────────────────────────────────────────────────
        if let Some(cat_name) = &data.category {
            if !cat_name.is_empty() {
                let cat_id: Uuid = sqlx::query_scalar(
                    "INSERT INTO categories (name) VALUES ($1) \
                     ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name \
                     RETURNING id",
                )
                .bind(cat_name)
                .fetch_one(&mut **tx)
                .await?;

                sqlx::query("UPDATE items SET category_id = $1 WHERE id = $2")
                    .bind(cat_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
            }
        }

        // ── Normalized tags ──────────────────────────────────────────────────────────
        for tag_name in &data.tags {
            if tag_name.is_empty() {
                continue;
            }
            let tag_id: Uuid = sqlx::query_scalar(
                "INSERT INTO tags (name) VALUES ($1) \
                 ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name \
                 RETURNING id",
            )
            .bind(tag_name)
            .fetch_one(&mut **tx)
            .await?;

            sqlx::query(
                "INSERT INTO item_tags (item_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(tag_id)
            .execute(&mut **tx)
            .await?;
        }

        // ── Container extension row ──────────────────────────────────────────────────
        if data.is_container {
            sqlx::query(
                "INSERT INTO container_properties \
                 (item_id, location_schema, max_capacity_cc, max_weight_grams, container_type_id) \
                 VALUES ($1, $2, $3, $4, $5) ON CONFLICT (item_id) DO NOTHING",
            )
            .bind(id)
            .bind(&data.location_schema)
            .bind(data.max_capacity_cc)
            .bind(data.max_weight_grams)
            .bind(data.container_type_id)
            .execute(&mut **tx)
            .await?;
        }

        // ── Fungible extension row ───────────────────────────────────────────────────
        if data.is_fungible {
            sqlx::query(
                "INSERT INTO fungible_properties (item_id, quantity, unit) \
                 VALUES ($1, $2, $3) ON CONFLICT (item_id) DO NOTHING",
            )
            .bind(id)
            .bind(data.fungible_quantity)
            .bind(&data.fungible_unit)
            .execute(&mut **tx)
            .await?;
        }

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
        let allowed_text_fields = ["name", "description", "condition", "currency"];
        let allowed_numeric_fields = [
            "weight_grams",
            "acquisition_cost", "current_value", "depreciation_rate",
        ];
        let allowed_jsonb_fields = ["coordinate", "dimensions", "metadata"];

        // Fields that ONLY write to extension/junction tables (no direct items UPDATE).
        // If all changes target these fields, we still need to touch items.updated_by.
        let extension_only_fields = [
            "tags", "location_schema", "max_capacity_cc", "max_weight_grams",
            "container_type_id", "fungible_unit", "fungible_quantity",
        ];
        let items_touched = data.changes.iter()
            .any(|c| !extension_only_fields.contains(&c.field.as_str()));

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
                // Bind None when new value is JSON null so DB stores SQL NULL, not 'null'::jsonb
                let jsonb_value: Option<&serde_json::Value> = if change.new.is_null() {
                    None
                } else {
                    Some(&change.new)
                };
                sqlx::query(&query)
                    .bind(jsonb_value)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;

            } else if field == "category" {
                // Normalize: get-or-create category row, update foreign key.
                let new_name = change.new.as_str().filter(|s| !s.is_empty());
                let cat_id: Option<Uuid> = if let Some(name) = new_name {
                    let cid: Uuid = sqlx::query_scalar(
                        "INSERT INTO categories (name) VALUES ($1) \
                         ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name \
                         RETURNING id",
                    )
                    .bind(name)
                    .fetch_one(&mut **tx)
                    .await?;
                    Some(cid)
                } else {
                    None
                };
                sqlx::query("UPDATE items SET category_id = $1, updated_by = $2 WHERE id = $3")
                    .bind(cat_id)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;

            } else if field == "tags" {
                // Normalize: replace all item_tags entries.
                let tags: Vec<String> = match serde_json::from_value(change.new.clone()) {
                    Ok(t) => t,
                    Err(e) => {
                        warn!(item_id = %id, field, error = %e, "projector: failed to deserialize tags, skipping tag update");
                        continue;
                    }
                };
                sqlx::query("DELETE FROM item_tags WHERE item_id = $1")
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
                for tag_name in &tags {
                    if tag_name.is_empty() {
                        continue;
                    }
                    let tag_id: Uuid = sqlx::query_scalar(
                        "INSERT INTO tags (name) VALUES ($1) \
                         ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name \
                         RETURNING id",
                    )
                    .bind(tag_name)
                    .fetch_one(&mut **tx)
                    .await?;
                    sqlx::query(
                        "INSERT INTO item_tags (item_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    )
                    .bind(id)
                    .bind(tag_id)
                    .execute(&mut **tx)
                    .await?;
                }

            } else if field == "max_capacity_cc" {
                let value = change.new.as_f64();
                sqlx::query(
                    "INSERT INTO container_properties (item_id, max_capacity_cc) VALUES ($1, $2) \
                     ON CONFLICT (item_id) DO UPDATE SET max_capacity_cc = EXCLUDED.max_capacity_cc",
                )
                .bind(id)
                .bind(value)
                .execute(&mut **tx)
                .await?;

            } else if field == "max_weight_grams" {
                let value = change.new.as_f64();
                sqlx::query(
                    "INSERT INTO container_properties (item_id, max_weight_grams) VALUES ($1, $2) \
                     ON CONFLICT (item_id) DO UPDATE SET max_weight_grams = EXCLUDED.max_weight_grams",
                )
                .bind(id)
                .bind(value)
                .execute(&mut **tx)
                .await?;

            } else if field == "container_type_id" {
                // Try to parse as UUID string first, then as JSON UUID value.
                // A JSON null legitimately clears the type, so only warn on
                // non-null values that fail to parse.
                let value: Option<Uuid> = change.new.as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .or_else(|| {
                        serde_json::from_value::<Option<Uuid>>(change.new.clone()).ok().flatten()
                    });
                if value.is_none() && !change.new.is_null() {
                    warn!(item_id = %id, raw = %change.new, "projector: container_type_id is non-null but could not be parsed as UUID, storing NULL");
                }
                sqlx::query(
                    "INSERT INTO container_properties (item_id, container_type_id) VALUES ($1, $2) \
                     ON CONFLICT (item_id) DO UPDATE SET container_type_id = EXCLUDED.container_type_id",
                )
                .bind(id)
                .bind(value)
                .execute(&mut **tx)
                .await?;

            } else if field == "fungible_unit" {
                let value = change.new.as_str().map(|s| s.to_string());
                sqlx::query(
                    "INSERT INTO fungible_properties (item_id, unit) VALUES ($1, $2) \
                     ON CONFLICT (item_id) DO UPDATE SET unit = EXCLUDED.unit",
                )
                .bind(id)
                .bind(&value)
                .execute(&mut **tx)
                .await?;

            } else if field == "fungible_quantity" {
                // Legacy: quantity changes should go through ItemQuantityAdjusted but
                // handle here for backward compat with older events.
                let value = change.new.as_i64().map(|v| v as i32);
                sqlx::query(
                    "INSERT INTO fungible_properties (item_id, quantity) VALUES ($1, $2) \
                     ON CONFLICT (item_id) DO UPDATE SET quantity = EXCLUDED.quantity",
                )
                .bind(id)
                .bind(value)
                .execute(&mut **tx)
                .await?;

            } else if field == "location_schema" {
                // Writes to container_properties, same as ContainerSchemaUpdated.
                // location_schema normally changes via ContainerSchemaUpdated, but handle
                // it here too so ItemUpdated events with this field don't silently diverge.
                let schema_value: Option<&serde_json::Value> = if change.new.is_null() {
                    None
                } else {
                    Some(&change.new)
                };
                sqlx::query(
                    "INSERT INTO container_properties (item_id, location_schema) VALUES ($1, $2) \
                     ON CONFLICT (item_id) DO UPDATE SET location_schema = EXCLUDED.location_schema",
                )
                .bind(id)
                .bind(schema_value)
                .execute(&mut **tx)
                .await?;

            } else if field == "is_container" {
                let value = change.new.as_bool()
                    .or_else(|| change.new.as_str().and_then(|s| s.parse::<bool>().ok()))
                    .unwrap_or_else(|| {
                        warn!(item_id = %id, raw = %change.new, "projector: is_container is not a bool, defaulting to false");
                        false
                    });
                sqlx::query("UPDATE items SET is_container = $1, updated_by = $2 WHERE id = $3")
                    .bind(value)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
                if value {
                    // Toggled on: ensure a container_properties row exists.
                    sqlx::query(
                        "INSERT INTO container_properties (item_id) VALUES ($1) ON CONFLICT DO NOTHING",
                    )
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
                } else {
                    // Toggled off: remove the extension row.
                    sqlx::query("DELETE FROM container_properties WHERE item_id = $1")
                        .bind(id)
                        .execute(&mut **tx)
                        .await?;
                }

            } else if field == "is_fungible" {
                let value = change.new.as_bool()
                    .or_else(|| change.new.as_str().and_then(|s| s.parse::<bool>().ok()))
                    .unwrap_or_else(|| {
                        warn!(item_id = %id, raw = %change.new, "projector: is_fungible is not a bool, defaulting to false");
                        false
                    });
                sqlx::query("UPDATE items SET is_fungible = $1, updated_by = $2 WHERE id = $3")
                    .bind(value)
                    .bind(actor_id)
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
                if value {
                    // Toggled on: ensure a fungible_properties row exists.
                    sqlx::query(
                        "INSERT INTO fungible_properties (item_id) VALUES ($1) ON CONFLICT DO NOTHING",
                    )
                    .bind(id)
                    .execute(&mut **tx)
                    .await?;
                } else {
                    // Toggled off: remove the extension row.
                    sqlx::query("DELETE FROM fungible_properties WHERE item_id = $1")
                        .bind(id)
                        .execute(&mut **tx)
                        .await?;
                }

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

        // Only touch updated_by when no branch above already wrote to the items table.
        if !items_touched {
            sqlx::query("UPDATE items SET updated_by = $1 WHERE id = $2")
                .bind(actor_id)
                .bind(id)
                .execute(&mut **tx)
                .await?;
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
        // Resolve destination: if to_container_id/to_path are None, look up current state from DB
        let (resolved_to_id, resolved_to_path) = match (data.to_container_id, data.to_path.as_ref()) {
            (Some(cid), Some(path)) => (cid, path.clone()),
            _ => {
                // Fetch current container info from the item row
                let row: Option<(Option<Uuid>, Option<String>)> = sqlx::query_as(
                    "SELECT parent_id, container_path::text FROM items WHERE id = $1",
                )
                .bind(id)
                .fetch_optional(&mut **tx)
                .await?;
                match row {
                    Some((Some(pid), Some(path))) => (pid, path),
                    _ => return Err(AppError::Internal(
                        "Cannot revert move: missing destination info and no current parent".into(),
                    )),
                }
            }
        };

        // Reuse move logic: move back to original container
        let move_data = ItemMovedData {
            from_container_id: Some(data.from_container_id),
            to_container_id: resolved_to_id,
            from_path: Some(data.from_path.clone()),
            to_path: resolved_to_path,
            coordinate: data.coordinate.clone(),
            from_coordinate: None,
        };
        Self::project_item_moved(tx, id, &move_data, actor_id).await
    }

    async fn project_item_deleted(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        actor_id: Uuid,
    ) -> AppResult<()> {
        let result = sqlx::query("UPDATE items SET is_deleted = TRUE, deleted_at = NOW(), updated_by = $1 WHERE id = $2")
            .bind(actor_id)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        if result.rows_affected() == 0 {
            tracing::warn!(item_id = %id, "project_item_deleted: UPDATE affected 0 rows — item may already be deleted or missing");
        }
        Ok(())
    }

    async fn project_item_restored(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        actor_id: Uuid,
    ) -> AppResult<()> {
        let result = sqlx::query("UPDATE items SET is_deleted = FALSE, deleted_at = NULL, updated_by = $1 WHERE id = $2")
            .bind(actor_id)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        if result.rows_affected() == 0 {
            tracing::warn!(item_id = %id, "project_item_restored: UPDATE affected 0 rows — item may be missing from projection");
        }
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
        // Dedup: only append if no existing entry has the same path.
        // PI-1: COALESCE is required because NULL || value = NULL in Postgres;
        // without it, the first image on a row with images = NULL is silently lost.
        sqlx::query(
            r#"
            UPDATE items
            SET images = CASE
                WHEN NOT EXISTS (
                    SELECT 1 FROM jsonb_array_elements(COALESCE(images, '[]'::jsonb)) AS elem
                    WHERE elem->>'path' = $1
                ) THEN COALESCE(images, '[]'::jsonb) || $4::jsonb
                ELSE images
            END,
            updated_by = $2
            WHERE id = $3
            "#,
        )
        .bind(&data.path)
        .bind(actor_id)
        .bind(id)
        .bind(&entry)
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
        // Dedup: only append if no existing entry has the same type+value.
        // PI-2: COALESCE guards against NULL external_codes (same pattern as images).
        sqlx::query(
            r#"
            UPDATE items
            SET external_codes = CASE
                WHEN NOT EXISTS (
                    SELECT 1 FROM jsonb_array_elements(COALESCE(external_codes, '[]'::jsonb)) AS elem
                    WHERE elem->>'type' = $1 AND elem->>'value' = $4
                ) THEN COALESCE(external_codes, '[]'::jsonb) || $5::jsonb
                ELSE external_codes
            END,
            updated_by = $2
            WHERE id = $3
            "#,
        )
        .bind(&data.code_type)
        .bind(actor_id)
        .bind(id)
        .bind(&data.value)
        .bind(&entry)
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
        // Quantity lives in fungible_properties.  Use UPSERT so replay is safe
        // even if the row was created after this event (unlikely but possible in replay).
        sqlx::query(
            "INSERT INTO fungible_properties (item_id, quantity) VALUES ($1, $2) \
             ON CONFLICT (item_id) DO UPDATE SET quantity = EXCLUDED.quantity",
        )
        .bind(id)
        .bind(data.new_qty)
        .execute(&mut **tx)
        .await?;

        sqlx::query("UPDATE items SET updated_by = $1 WHERE id = $2")
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
        // Location schema lives in container_properties.  UPSERT for idempotent replay.
        // When new_schema is JSON null (produced by undo of an initial schema set), bind as
        // SQL NULL so the column is properly cleared rather than storing 'null'::jsonb.
        let schema_value: Option<&serde_json::Value> = if data.new_schema.is_null() {
            None
        } else {
            Some(&data.new_schema)
        };
        sqlx::query(
            "INSERT INTO container_properties (item_id, location_schema) VALUES ($1, $2) \
             ON CONFLICT (item_id) DO UPDATE SET location_schema = EXCLUDED.location_schema",
        )
        .bind(id)
        .bind(schema_value)
        .execute(&mut **tx)
        .await?;

        // Rename children's coordinates when labels are renamed.
        // Topological sort (Kahn's algorithm) so rename chains of any length are
        // processed correctly.  Rule: entry F must be processed AFTER entry E whenever
        // E.old_label == F.new_label (otherwise F would move items into E's source
        // slot, causing double-movement for chained renames like {A→B, B→C, C→D}).
        //
        // "Not blocked" in each round = entries whose new_label does not appear as the
        // old_label of any remaining entry.  These are safe to defer no longer and are
        // emitted into `sorted` first.  Remaining entries are processed in the next round.
        let mut remaining: Vec<(&String, &String)> = data.label_renames.iter().collect();
        let mut sorted: Vec<(&String, &String)> = Vec::with_capacity(remaining.len());
        while !remaining.is_empty() {
            let old_labels: std::collections::HashSet<&str> =
                remaining.iter().map(|(old, _)| old.as_str()).collect();
            let (ready, blocked): (Vec<_>, Vec<_>) = remaining
                .into_iter()
                .partition(|(_, new)| !old_labels.contains(new.as_str()));
            if ready.is_empty() {
                // Cycle in renames — process blocked in any order to avoid infinite loop.
                sorted.extend(blocked);
                break;
            }
            sorted.extend(ready);
            remaining = blocked;
        }
        for (old_label, new_label) in sorted {
            sqlx::query(
                "UPDATE items SET coordinate = jsonb_set(coordinate, '{value}', to_jsonb($1::text)) \
                 WHERE parent_id = $2 AND coordinate->>'type' = 'abstract' AND coordinate->>'value' = $3",
            )
            .bind(new_label)
            .bind(id)
            .bind(old_label)
            .execute(&mut **tx)
            .await?;
        }

        sqlx::query("UPDATE items SET updated_by = $1 WHERE id = $2")
            .bind(actor_id)
            .bind(id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    async fn project_barcode_assigned(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        data: &ItemBarcodeAssignedData,
        actor_id: Uuid,
    ) -> AppResult<()> {
        let barcode: Option<&str> = if data.barcode.is_empty() {
            None // empty string means "clear barcode"
        } else {
            Some(&data.barcode)
        };

        sqlx::query(
            "UPDATE items SET system_barcode = $1, updated_by = $2 WHERE id = $3",
        )
        .bind(barcode)
        .bind(actor_id)
        .bind(id)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Rebuild all projections by replaying the entire event store.
    /// WARNING: This truncates the items table (except seed data) and replays everything.
    /// Uses a PostgreSQL advisory lock to prevent concurrent rebuilds.
    /// ES-2/RM-1: Fetches events in batches (1 000 at a time) to keep memory usage bounded.
    /// EH-3: Skips individual deserialization failures with a warning instead of aborting.
    pub async fn rebuild_all(pool: &PgPool) -> AppResult<u64> {
        let mut tx = pool.begin().await?;

        // CONC-3: Limit how long we will wait to acquire the advisory lock.  This
        // prevents a queued rebuild from blocking indefinitely if another rebuild is
        // still running, and surfaces a clear error instead of a silent hang.
        sqlx::query("SET LOCAL lock_timeout = '10s'")
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to set lock_timeout: {e}")))?;

        // Advisory lock prevents concurrent rebuilds
        sqlx::query("SELECT pg_advisory_xact_lock(7307942)")
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                if e.to_string().contains("lock timeout") || e.to_string().contains("55P03") {
                    AppError::Conflict("A projection rebuild is already in progress".into())
                } else {
                    AppError::Database(e)
                }
            })?;

        // Delete all non-seed items; seed rows will be re-inserted via ON CONFLICT below.
        // Cascade constraints on container_properties, fungible_properties and item_tags
        // will clean up those tables automatically.
        sqlx::query("DELETE FROM items WHERE id NOT IN ($1, $2)")
            .bind(ROOT_ID)
            .bind(USERS_ID)
            .execute(&mut *tx)
            .await?;

        // Clean extension tables for seed rows so they are rebuilt from events.
        // (The CASCADE above only fires for deleted items; seed rows are kept.)
        sqlx::query("DELETE FROM container_properties WHERE item_id IN ($1, $2)")
            .bind(ROOT_ID)
            .bind(USERS_ID)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM fungible_properties WHERE item_id IN ($1, $2)")
            .bind(ROOT_ID)
            .bind(USERS_ID)
            .execute(&mut *tx)
            .await?;

        // Clear normalized reference tables so they are rebuilt from events
        // (categories/tags have no seed data).
        sqlx::query("DELETE FROM item_tags").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM tags").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM categories").execute(&mut *tx).await?;

        const BATCH: i64 = 1_000;
        let mut last_id: i64 = 0;
        let mut total: u64 = 0;
        let mut skipped: u64 = 0;

        loop {
            // ES-2/RM-1: Load one batch instead of the entire event store at once.
            let batch = sqlx::query_as::<_, StoredEvent>(
                r#"
                SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata,
                       actor_id, created_at, sequence_number, schema_version
                FROM event_store
                WHERE id > $1
                ORDER BY id ASC
                LIMIT $2
                "#,
            )
            .bind(last_id)
            .bind(BATCH)
            .fetch_all(&mut *tx)
            .await?;

            if batch.is_empty() {
                break;
            }

            for stored in &batch {
                last_id = stored.id;

                let domain_event: DomainEvent = match serde_json::from_value(stored.event_data.clone()) {
                    Ok(e) => e,
                    // EH-3: Log and skip individual bad events instead of aborting the whole rebuild.
                    Err(e) => {
                        tracing::warn!(
                            event_id = %stored.event_id,
                            event_type = %stored.event_type,
                            error = %e,
                            "rebuild_all: skipping undeserializable event"
                        );
                        skipped += 1;
                        continue;
                    }
                };

                let actor = stored.actor_id.unwrap_or(Uuid::nil());
                Self::apply(&mut tx, stored.aggregate_id, &domain_event, actor).await?;
                total += 1;
            }
        }

        if skipped > 0 {
            tracing::warn!(skipped, "rebuild_all: {skipped} events skipped due to deserialization errors");
        }

        tx.commit().await?;
        Ok(total)
    }
}
