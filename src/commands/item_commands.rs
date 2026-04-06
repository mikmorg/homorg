use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;

use crate::constants::{MAX_EXTERNAL_CODES, MAX_CODE_VALUE_LEN, MAX_CODE_TYPE_LEN};
use crate::queries::item_queries::ITEM_FULL_SELECT;
use crate::errors::{AppError, AppResult};
use crate::events::projector::Projector;
use crate::events::store::EventStore;
use crate::models::event::*;
use crate::models::item::*;

/// Command handler for item write operations.
#[derive(Clone)]
pub struct ItemCommands {
    pool: PgPool,
    event_store: EventStore,
}

impl ItemCommands {
    pub fn new(pool: PgPool, event_store: EventStore) -> Self {
        Self { pool, event_store }
    }

    /// Verify an item exists and is not deleted (within a transaction).
    async fn verify_item_exists(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: Uuid,
    ) -> AppResult<()> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM items WHERE id = $1 AND is_deleted = FALSE)",
        )
        .bind(item_id)
        .fetch_one(&mut **tx)
        .await?;

        if !exists {
            return Err(AppError::NotFound(format!("Item {item_id} not found")));
        }
        Ok(())
    }

    /// Create a new item and place it inside a parent container.
    pub async fn create_item(
        &self,
        id: Uuid,
        req: &CreateItemRequest,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;
        let stored = self.create_item_in_tx(&mut tx, id, req, actor_id, metadata).await?;
        tx.commit().await?;
        Ok(stored)
    }

    /// Create a new item within an existing transaction (for batch operations).
    pub async fn create_item_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        req: &CreateItemRequest,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        // Validate parent exists and is a container
        let parent = sqlx::query_as::<_, (Uuid, bool, Option<String>)>(
            "SELECT id, is_container, container_path::text FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(req.parent_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Parent container {} not found", req.parent_id)))?;

        if !parent.1 {
            return Err(AppError::BadRequest(format!(
                "Item {} is not a container", req.parent_id
            )));
        }

        let is_container = req.is_container.unwrap_or(false);
        let is_fungible = req.is_fungible.unwrap_or(false);

        // Mutual-exclusivity: an item cannot be both a container and fungible.
        if is_container && is_fungible {
            return Err(AppError::BadRequest(
                "An item cannot be both a container and fungible".into(),
            ));
        }

        let system_barcode = req.system_barcode.clone();

        // If a barcode is provided, check it is unique (prevents silent collision on concurrent requests).
        // M-5: exclude soft-deleted items — barcode reuse after deletion is allowed.
        if let Some(ref bc) = system_barcode {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM items WHERE system_barcode = $1 AND is_deleted = FALSE)",
            )
            .bind(bc)
            .fetch_one(&mut **tx)
            .await?;

            if exists {
                return Err(AppError::Conflict(format!("Barcode {bc} already exists")));
            }
        }

        // Derive immutable node_id from item UUID and build path
        let node_id = uuid_to_node_id(&id);
        let parent_path = parent.2.unwrap_or_else(|| "n_root".to_string());
        let container_path = format!("{}.{}", parent_path, node_id);

        let external_codes: Vec<serde_json::Value> = req.external_codes
            .as_ref()
            .map(|codes| codes.iter().map(|c| serde_json::json!({"type": c.code_type, "value": c.value})).collect())
            .unwrap_or_default();

        let event = DomainEvent::ItemCreated(Box::new(ItemCreatedData {
            system_barcode: system_barcode.clone(),
            node_id,
            name: req.name.clone(),
            description: req.description.clone(),
            category: req.category.clone(),
            tags: req.tags.clone().unwrap_or_default(),
            is_container,
            container_path,
            parent_id: req.parent_id,
            coordinate: req.coordinate.clone(),
            location_schema: req.location_schema.clone(),
            max_capacity_cc: req.max_capacity_cc,
            max_weight_grams: req.max_weight_grams,
            dimensions: req.dimensions.clone(),
            weight_grams: req.weight_grams,
            is_fungible,
            fungible_quantity: req.fungible_quantity,
            fungible_unit: req.fungible_unit.clone(),
            external_codes,
            condition: req.condition.clone(),
            currency: req.currency.clone(),
            acquisition_date: req.acquisition_date.map(|d| d.to_string()),
            acquisition_cost: req.acquisition_cost,
            current_value: req.current_value,
            depreciation_rate: req.depreciation_rate,
            warranty_expiry: req.warranty_expiry.map(|d| d.to_string()),
            metadata: req.metadata.clone().unwrap_or_else(|| serde_json::json!({})),
            // DI-2: Record creation timestamp in the event so rebuild_all restores original dates.
            created_at: Some(Utc::now()),
            container_type_id: req.container_type_id,
        }));

        let stored = self.event_store.append_in_tx(tx, id, &event, actor_id, metadata).await?;
        Projector::apply(tx, id, &event, actor_id).await.map_err(|e| {
            // DI-6: node_id UNIQUE collision → 409 instead of opaque 500.
            // DI-7: system_barcode concurrent-insert race → 409 instead of opaque 500.
            if let AppError::Database(sqlx::Error::Database(ref db_err)) = e {
                if db_err.constraint() == Some("items_node_id_key") {
                    return AppError::Conflict(
                        "node_id collision detected — retry with a new item UUID".into(),
                    );
                }
                if db_err.constraint() == Some("idx_items_system_barcode_live") {
                    return AppError::Conflict(format!(
                        "Barcode '{}' already exists",
                        system_barcode.as_deref().unwrap_or("(unknown)")
                    ));
                }
            }
            e
        })?;

        Ok(stored)
    }

    /// Partially update item metadata fields.
    pub async fn update_item(
        &self,
        item_id: Uuid,
        req: &UpdateItemRequest,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;

        // Fetch current state to compute diffs (inside tx to prevent TOCTOU).
        // Use the full JOIN query so extension-table fields (fungible_unit, location_schema…)
        // are populated on the Item struct.
        let current = sqlx::query_as::<_, Item>(
            &format!("SELECT {ITEM_FULL_SELECT} WHERE i.id = $1 AND i.is_deleted = FALSE FOR UPDATE OF i"),
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        let mut changes = Vec::new();

        macro_rules! diff_field {
            ($field:ident, $current_val:expr) => {
                if let Some(ref new_val) = req.$field {
                    let old = serde_json::to_value(&$current_val).unwrap_or(serde_json::Value::Null);
                    let new = serde_json::to_value(new_val).unwrap_or(serde_json::Value::Null);
                    if old != new {
                        changes.push(FieldChange {
                            field: stringify!($field).to_string(),
                            old,
                            new,
                        });
                    }
                }
            };
        }

        // Nullable field diff for Option<Option<T>>:
        //   None          → field absent from JSON → no change
        //   Some(None)    → explicit null in JSON  → clear to NULL
        //   Some(Some(v)) → value present in JSON  → set to v
        macro_rules! diff_nullable_field {
            ($field:ident, $current_val:expr) => {
                if let Some(ref inner) = req.$field {
                    let old = serde_json::to_value(&$current_val).unwrap_or(serde_json::Value::Null);
                    let new = serde_json::to_value(inner).unwrap_or(serde_json::Value::Null);
                    if old != new {
                        changes.push(FieldChange {
                            field: stringify!($field).to_string(),
                            old,
                            new,
                        });
                    }
                }
            };
        }

        // Nullable numeric diff for Option<Option<f64>>:
        //   None          → field absent → no change
        //   Some(None)    → explicit null → clear to NULL
        //   Some(Some(v)) → value present → compare with rounding
        // CB-3: Decimal→f64 rounding to 6dp suppresses binary-float noise.
        macro_rules! diff_nullable_numeric {
            ($field:ident, $current_val:expr) => {
                if let Some(ref inner) = req.$field {
                    use rust_decimal::prelude::ToPrimitive;
                    let old_f64: Option<f64> = $current_val.as_ref().and_then(|d: &rust_decimal::Decimal| d.to_f64());
                    match inner {
                        None => {
                            // Clearing to NULL — only record change if currently set
                            if old_f64.is_some() {
                                changes.push(FieldChange {
                                    field: stringify!($field).to_string(),
                                    old: serde_json::to_value(&old_f64).unwrap_or(serde_json::Value::Null),
                                    new: serde_json::Value::Null,
                                });
                            }
                        }
                        Some(new_f64) => {
                            let old_r = old_f64.map(|f| (f * 1_000_000.0).round() / 1_000_000.0);
                            let new_r = (*new_f64 * 1_000_000.0).round() / 1_000_000.0;
                            if old_r.is_none_or(|o| (o - new_r).abs() > 1e-9) {
                                changes.push(FieldChange {
                                    field: stringify!($field).to_string(),
                                    old: serde_json::to_value(&old_f64).unwrap_or(serde_json::Value::Null),
                                    new: serde_json::to_value(new_f64).unwrap_or(serde_json::Value::Null),
                                });
                            }
                        }
                    }
                }
            };
        }

        diff_field!(name, current.name);
        diff_field!(description, current.description);
        diff_field!(category, current.category);
        diff_field!(tags, current.tags);
        diff_field!(is_container, current.is_container);
        diff_field!(coordinate, current.coordinate);
        diff_nullable_numeric!(max_capacity_cc, current.max_capacity_cc);
        diff_nullable_numeric!(max_weight_grams, current.max_weight_grams);
        diff_field!(dimensions, current.dimensions);
        diff_nullable_numeric!(weight_grams, current.weight_grams);
        diff_nullable_field!(condition, current.condition);
        diff_nullable_field!(currency, current.currency);
        diff_nullable_field!(acquisition_date, current.acquisition_date);
        // B5: Decimal fields use exact equality; serde serializes them as strings.
        macro_rules! diff_nullable_decimal {
            ($field:ident, $current_val:expr) => {
                if let Some(ref inner) = req.$field {
                    match inner {
                        None => {
                            if $current_val.is_some() {
                                changes.push(FieldChange {
                                    field: stringify!($field).to_string(),
                                    old: serde_json::to_value($current_val).unwrap_or(serde_json::Value::Null),
                                    new: serde_json::Value::Null,
                                });
                            }
                        }
                        Some(new_val) => {
                            if $current_val.as_ref() != Some(new_val) {
                                changes.push(FieldChange {
                                    field: stringify!($field).to_string(),
                                    old: serde_json::to_value($current_val).unwrap_or(serde_json::Value::Null),
                                    new: serde_json::to_value(new_val).unwrap_or(serde_json::Value::Null),
                                });
                            }
                        }
                    }
                }
            };
        }
        diff_nullable_decimal!(acquisition_cost, current.acquisition_cost);
        diff_nullable_decimal!(current_value, current.current_value);
        diff_nullable_decimal!(depreciation_rate, current.depreciation_rate);
        diff_nullable_field!(warranty_expiry, current.warranty_expiry);
        diff_field!(metadata, current.metadata);
        diff_nullable_field!(system_barcode, current.system_barcode);
        diff_field!(external_codes, current.external_codes);
        diff_field!(is_fungible, current.is_fungible);
        diff_nullable_field!(fungible_unit, current.fungible_unit);
        diff_nullable_field!(container_type_id, current.container_type_id);

        // Mutual exclusivity: reject if the update would result in is_container AND is_fungible both being true.
        let will_be_container = req.is_container.unwrap_or(current.is_container);
        let will_be_fungible  = req.is_fungible.unwrap_or(current.is_fungible);
        if will_be_container && will_be_fungible {
            return Err(AppError::BadRequest(
                "An item cannot be both a container and fungible".into(),
            ));
        }

        if changes.is_empty() {
            return Err(AppError::BadRequest("No changes detected".into()));
        }

        // Guard: cannot toggle is_container to false if children exist
        if let Some(is_container_change) = changes.iter().find(|c| c.field == "is_container") {
            if is_container_change.new == serde_json::Value::Bool(false) && current.is_container {
                let child_count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM items WHERE parent_id = $1 AND is_deleted = FALSE",
                )
                .bind(item_id)
                .fetch_one(&mut *tx)
                .await?;

                if child_count > 0 {
                    return Err(AppError::Conflict(format!(
                        "Cannot unset is_container: {child_count} active children exist"
                    )));
                }
            }
        }

        let event = DomainEvent::ItemUpdated(ItemUpdatedData { changes });

        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await.map_err(|e| {
            if let AppError::Database(sqlx::Error::Database(ref db_err)) = e {
                if db_err.constraint() == Some("idx_items_system_barcode_live") {
                    return AppError::Conflict("Barcode already exists".into());
                }
            }
            e
        })?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Move an item to a different container.
    pub async fn move_item(
        &self,
        item_id: Uuid,
        req: &MoveItemRequest,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;
        let stored = self.move_item_in_tx(&mut tx, item_id, req, actor_id, metadata).await?;
        tx.commit().await?;
        Ok(stored)
    }

    /// Move an item within an existing transaction (for batch operations).
    pub async fn move_item_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: Uuid,
        req: &MoveItemRequest,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let item = sqlx::query_as::<_, (Uuid, Option<Uuid>, Option<String>, String, bool, Option<serde_json::Value>)>(
            "SELECT id, parent_id, container_path::text, node_id, is_container, coordinate FROM items WHERE id = $1 AND is_deleted = FALSE FOR UPDATE",
        )
        .bind(item_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        let dest = sqlx::query_as::<_, (Uuid, bool, Option<String>)>(
            "SELECT id, is_container, container_path::text FROM items WHERE id = $1 AND is_deleted = FALSE FOR UPDATE",
        )
        .bind(req.container_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Destination container {} not found", req.container_id)))?;

        if !dest.1 {
            return Err(AppError::BadRequest(format!(
                "Destination {} is not a container", req.container_id
            )));
        }

        // H-4: Idempotency — reject no-op moves (item is already in the target container).
        if item.1 == Some(req.container_id) {
            return Err(AppError::BadRequest(
                "Item is already in this container".into(),
            ));
        }

        // Circular reference check: destination must not be a descendant of the moved item.
        // Require paths to be present — if either is NULL we cannot verify safety.
        if item.4 {
            // item is_container
            match (&item.2, &dest.2) {
                (Some(item_path), Some(dest_path)) => {
                    if dest_path == item_path || dest_path.starts_with(&format!("{item_path}.")) {
                        return Err(AppError::Conflict(
                            "Cannot move a container into its own descendant".into(),
                        ));
                    }
                }
                _ => {
                    return Err(AppError::Internal(
                        "Cannot verify move safety: container_path is missing for item or destination".into(),
                    ));
                }
            }
        }

        let dest_path = dest.2.unwrap_or_else(|| "n_root".to_string());
        let new_path = format!("{}.{}", dest_path, item.3);

        let event = DomainEvent::ItemMoved(ItemMovedData {
            from_container_id: item.1,
            to_container_id: req.container_id,
            from_path: item.2.clone(),
            to_path: new_path,
            coordinate: req.coordinate.clone(),
            from_coordinate: item.5,
        });

        let stored = self.event_store.append_in_tx(tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(tx, item_id, &event, actor_id).await?;

        Ok(stored)
    }

    /// Soft-delete an item.
    /// Rejects deletion of non-empty containers (children must be moved or deleted first).
    pub async fn delete_item(
        &self,
        item_id: Uuid,
        reason: Option<String>,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;

        // Check item exists and whether it's a container (inside tx to prevent TOCTOU)
        let row = sqlx::query_as::<_, (Uuid, bool)>(
            "SELECT id, is_container FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        // Guard: prevent deleting a non-empty container
        if row.1 {
            let child_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM items WHERE parent_id = $1 AND is_deleted = FALSE",
            )
            .bind(item_id)
            .fetch_one(&mut *tx)
            .await?;

            if child_count > 0 {
                return Err(AppError::Conflict(format!(
                    "Cannot delete non-empty container ({child_count} active children). Move or delete children first."
                )));
            }
        }

        let event = DomainEvent::ItemDeleted(ItemDeletedData { reason });

        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Restore a soft-deleted item.
    /// Validates that the parent container still exists and is active.
    pub async fn restore_item(
        &self,
        item_id: Uuid,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;

        let item = sqlx::query_as::<_, (Uuid, bool, Option<Uuid>)>(
            "SELECT id, is_deleted, parent_id FROM items WHERE id = $1",
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        if !item.1 {
            return Err(AppError::BadRequest("Item is not deleted".into()));
        }

        // Verify parent container still exists and is active
        if let Some(parent_id) = item.2 {
            let parent_ok: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM items WHERE id = $1 AND is_deleted = FALSE AND is_container = TRUE)",
            )
            .bind(parent_id)
            .fetch_one(&mut *tx)
            .await?;

            if !parent_ok {
                return Err(AppError::Conflict(
                    "Cannot restore item: parent container is deleted or missing. Move to an active container first.".into(),
                ));
            }
        }

        let event = DomainEvent::ItemRestored(ItemRestoredData { from_event_id: None });

        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Add an image to an item.
    pub async fn add_image(
        &self,
        item_id: Uuid,
        path: String,
        caption: Option<String>,
        order: i32,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let event = DomainEvent::ItemImageAdded(ItemImageAddedData { path, caption, order });

        let mut tx = self.pool.begin().await?;
        // IMG-1 + H-3: Check image count inside the transaction with FOR UPDATE
        // to prevent concurrent uploads from exceeding the limit.
        let image_count: i64 = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_array_length(images), 0)::bigint FROM items WHERE id = $1 AND is_deleted = FALSE FOR UPDATE",
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        if image_count as usize >= crate::constants::MAX_IMAGES_PER_ITEM {
            return Err(AppError::BadRequest(format!(
                "Item already has {image_count} images (maximum {})",
                crate::constants::MAX_IMAGES_PER_ITEM
            )));
        }

        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Remove an image from an item.
    pub async fn remove_image(
        &self,
        item_id: Uuid,
        path: String,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;
        self.verify_item_exists(&mut tx, item_id).await?;

        // Look up caption/order for this image so undo can restore it.
        // G3: FOR UPDATE serializes concurrent remove+add to prevent races around MAX_IMAGES.
        let images_json: serde_json::Value = sqlx::query_scalar(
            "SELECT images FROM items WHERE id = $1 AND is_deleted = FALSE FOR UPDATE",
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;
        let images: Vec<crate::models::item::ImageEntry> =
            serde_json::from_value(images_json)
                .map_err(|e| AppError::Internal(format!("Failed to parse images JSON for item {item_id}: {e}")))?;
        let (caption, order) = images
            .iter()
            .find(|e| e.path == path)
            .map(|e| (e.caption.clone(), Some(e.order)))
            .ok_or_else(|| AppError::NotFound(format!("Image '{}' not found on item {item_id}", path)))?;

        let event = DomainEvent::ItemImageRemoved(ItemImageRemovedData { path, caption, order });
        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Remove an image by index (TOCTOU-safe: resolves index within the transaction).
    /// Returns (StoredEvent, path) so the caller can clean up the file.
    pub async fn remove_image_by_index(
        &self,
        item_id: Uuid,
        index: usize,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<(StoredEvent, String)> {
        let mut tx = self.pool.begin().await?;
        self.verify_item_exists(&mut tx, item_id).await?;

        // Resolve image path inside the transaction.
        // G3: FOR UPDATE serializes concurrent remove+add to prevent races around MAX_IMAGES.
        let images_json: serde_json::Value = sqlx::query_scalar(
            "SELECT images FROM items WHERE id = $1 AND is_deleted = FALSE FOR UPDATE",
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        let images: Vec<crate::models::item::ImageEntry> = serde_json::from_value(images_json)
            .map_err(|_| AppError::Internal("Failed to parse images".into()))?;

        let entry = images
            .get(index)
            .ok_or_else(|| AppError::NotFound(format!("Image index {index} not found")))?;

        let path = entry.path.clone();
        let caption = entry.caption.clone();
        let order = Some(entry.order);
        let event = DomainEvent::ItemImageRemoved(ItemImageRemovedData { path: path.clone(), caption, order });

        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok((stored, path))
    }

    /// Add an external code (UPC, EAN, ISBN) to an item.
    pub async fn add_external_code(
        &self,
        item_id: Uuid,
        code_type: String,
        value: String,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        // CB-5: Validate lengths before opening a transaction (fast rejection).
        if code_type.len() > MAX_CODE_TYPE_LEN {
            return Err(AppError::BadRequest(format!(
                "code_type exceeds {MAX_CODE_TYPE_LEN} chars"
            )));
        }
        if value.len() > MAX_CODE_VALUE_LEN {
            return Err(AppError::BadRequest(format!(
                "external code value exceeds {MAX_CODE_VALUE_LEN} chars"
            )));
        }

        let mut tx = self.pool.begin().await?;
        self.verify_item_exists(&mut tx, item_id).await?;

        // CB-5: Enforce max external code count inside the transaction to prevent TOCTOU.
        // jsonb_array_length returns INT4, so decode as i32.
        let current_count: i32 = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_array_length(external_codes), 0) FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await?;
        if current_count >= MAX_EXTERNAL_CODES as i32 {
            return Err(AppError::BadRequest(format!(
                "Cannot add external code: item already has {current_count} external codes (max {MAX_EXTERNAL_CODES})"
            )));
        }

        // I5: Normalize code_type to uppercase so "upc", "UPC", "Upc" all resolve to the same code.
        let code_type = code_type.to_uppercase();
        let event = DomainEvent::ItemExternalCodeAdded(ExternalCodeData { code_type, value });
        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Remove an external code from an item.
    pub async fn remove_external_code(
        &self,
        item_id: Uuid,
        code_type: String,
        value: String,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;
        self.verify_item_exists(&mut tx, item_id).await?;

        // CB-6: Check that the code actually exists before emitting a removal event.
        // Without this guard a caller could litter the event store with phantom
        // ItemExternalCodeRemoved events for codes that were never present.
        let codes_json: serde_json::Value = sqlx::query_scalar(
            "SELECT COALESCE(external_codes, '[]'::jsonb) FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await?;

        let exists = codes_json
            .as_array()
            .map(|arr| {
                arr.iter().any(|c| {
                    c.get("type").and_then(|v| v.as_str()) == Some(code_type.as_str())
                        && c.get("value").and_then(|v| v.as_str()) == Some(value.as_str())
                })
            })
            .unwrap_or(false);

        if !exists {
            return Err(AppError::NotFound(format!(
                "External code '{code_type}:{value}' not found on item {item_id}"
            )));
        }

        // I5: Normalize to uppercase to match the storage convention applied on add.
        let code_type = code_type.to_uppercase();
        let event = DomainEvent::ItemExternalCodeRemoved(ExternalCodeData { code_type, value });
        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Adjust fungible quantity.
    pub async fn adjust_quantity(
        &self,
        item_id: Uuid,
        req: &AdjustQuantityRequest,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;

        // QA-1: Use FOR UPDATE to serialize concurrent quantity adjustments so that
        // the old_qty recorded in each event reflects the true prior state, preventing
        // incorrect undo behavior when two callers write simultaneously.
        let current = sqlx::query_as::<_, (bool, Option<i32>)>(
            "SELECT i.is_fungible, fp.quantity FROM items i LEFT JOIN fungible_properties fp ON fp.item_id = i.id WHERE i.id = $1 AND i.is_deleted = FALSE FOR UPDATE OF i",
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        if !current.0 {
            return Err(AppError::BadRequest("Item is not fungible".into()));
        }

        // CB-5: Validate non-negative quantity here rather than letting the DB CHECK constraint
        // surface as an opaque 500 error.
        if req.new_quantity < 0 {
            return Err(AppError::BadRequest(format!(
                "New quantity must be >= 0 (got {})", req.new_quantity
            )));
        }

        let event = DomainEvent::ItemQuantityAdjusted(QuantityAdjustedData {
            old_qty: current.1,
            new_qty: req.new_quantity,
            reason: req.reason.clone(),
        });

        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Update a container's location schema.
    pub async fn update_container_schema(
        &self,
        container_id: Uuid,
        new_schema: serde_json::Value,
        label_renames: std::collections::HashMap<String, String>,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;

        let current_schema: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT cp.location_schema FROM items i LEFT JOIN container_properties cp ON cp.item_id = i.id WHERE i.id = $1 AND i.is_container = TRUE AND i.is_deleted = FALSE",
        )
        .bind(container_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Container {container_id} not found")))?;

        let event = DomainEvent::ContainerSchemaUpdated(ContainerSchemaUpdatedData {
            old_schema: current_schema,
            new_schema,
            label_renames,
        });

        let stored = self.event_store.append_in_tx(&mut tx, container_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, container_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }
}

/// Derive an immutable, LTREE-safe node ID from an item's UUID.
/// Produces labels like `n_4a8b3c1d2e3f` — first NODE_ID_HEX_LEN hex chars of the UUID
/// prefixed with `n_`. 12 hex chars = 48 bits of entropy → birthday collision at ~1%
/// requires ~16 million items, well beyond household scale. LTREE labels must match
/// `[A-Za-z_][A-Za-z0-9_]*`, so the `n_` prefix ensures the label never starts with a digit.
/// The UNIQUE constraint on `node_id` catches any collision at INSERT time.
pub fn uuid_to_node_id(id: &Uuid) -> String {
    use crate::constants::NODE_ID_HEX_LEN;
    format!("n_{}", &id.simple().to_string()[..NODE_ID_HEX_LEN])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_starts_with_n_prefix() {
        let id = Uuid::new_v4();
        let node = uuid_to_node_id(&id);
        assert!(node.starts_with("n_"));
    }

    #[test]
    fn node_id_has_correct_length() {
        use crate::constants::NODE_ID_HEX_LEN;
        let id = Uuid::new_v4();
        let node = uuid_to_node_id(&id);
        // "n_" (2 chars) + NODE_ID_HEX_LEN hex chars
        assert_eq!(node.len(), 2 + NODE_ID_HEX_LEN);
    }

    #[test]
    fn node_id_is_deterministic() {
        let id = Uuid::new_v4();
        assert_eq!(uuid_to_node_id(&id), uuid_to_node_id(&id));
    }

    #[test]
    fn different_uuids_produce_different_node_ids() {
        let a = uuid_to_node_id(&Uuid::new_v4());
        let b = uuid_to_node_id(&Uuid::new_v4());
        assert_ne!(a, b);
    }

    #[test]
    fn node_id_is_ltree_safe() {
        let id = Uuid::new_v4();
        let node = uuid_to_node_id(&id);
        // LTREE labels: first char must be letter or underscore, rest alphanumeric or underscore
        let mut chars = node.chars();
        let first = chars.next().unwrap();
        assert!(first.is_ascii_alphabetic() || first == '_');
        for c in chars {
            assert!(
                c.is_ascii_alphanumeric() || c == '_',
                "invalid ltree char: {c}"
            );
        }
    }
}
