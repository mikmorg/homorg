use sqlx::PgPool;
use uuid::Uuid;

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

        let system_barcode = req.system_barcode.clone()
            .ok_or_else(|| AppError::BadRequest("system_barcode is required".into()))?;

        // Check barcode uniqueness
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM items WHERE system_barcode = $1)",
        )
        .bind(&system_barcode)
        .fetch_one(&mut **tx)
        .await?;

        if exists {
            return Err(AppError::Conflict(format!(
                "Barcode {} already exists", system_barcode
            )));
        }

        // Derive immutable node_id from item UUID and build path
        let node_id = uuid_to_node_id(&id);
        let parent_path = parent.2.unwrap_or_else(|| "n_root".to_string());
        let container_path = format!("{}.{}", parent_path, node_id);

        let is_container = req.is_container.unwrap_or(false);
        let is_fungible = req.is_fungible.unwrap_or(false);

        let external_codes: Vec<serde_json::Value> = req.external_codes
            .as_ref()
            .map(|codes| codes.iter().map(|c| serde_json::json!({"type": c.code_type, "value": c.value})).collect())
            .unwrap_or_default();

        let event = DomainEvent::ItemCreated(ItemCreatedData {
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
            acquisition_date: req.acquisition_date.map(|d| d.to_string()),
            acquisition_cost: req.acquisition_cost,
            current_value: req.current_value,
            depreciation_rate: req.depreciation_rate,
            warranty_expiry: req.warranty_expiry.map(|d| d.to_string()),
            metadata: req.metadata.clone().unwrap_or_else(|| serde_json::json!({})),
        });

        let stored = self.event_store.append_in_tx(tx, id, &event, actor_id, metadata).await?;
        Projector::apply(tx, id, &event, actor_id).await?;

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
        // Fetch current state to compute diffs
        let current = sqlx::query_as::<_, Item>(
            "SELECT * FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
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

        // Numeric diff: compare as f64 to avoid Decimal (string "10.50") vs f64 (number 10.5) mismatch
        macro_rules! diff_numeric {
            ($field:ident, $current_val:expr) => {
                if let Some(ref new_val) = req.$field {
                    use rust_decimal::prelude::ToPrimitive;
                    let old_f64: Option<f64> = $current_val.as_ref().and_then(|d: &rust_decimal::Decimal| d.to_f64());
                    let new_f64: f64 = *new_val;
                    if old_f64.map_or(true, |o| (o - new_f64).abs() > f64::EPSILON) {
                        changes.push(FieldChange {
                            field: stringify!($field).to_string(),
                            old: serde_json::to_value(&old_f64).unwrap_or(serde_json::Value::Null),
                            new: serde_json::to_value(new_f64).unwrap_or(serde_json::Value::Null),
                        });
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
        diff_field!(location_schema, current.location_schema);
        diff_numeric!(max_capacity_cc, current.max_capacity_cc);
        diff_numeric!(max_weight_grams, current.max_weight_grams);
        diff_field!(dimensions, current.dimensions);
        diff_numeric!(weight_grams, current.weight_grams);
        diff_field!(condition, current.condition);
        diff_field!(acquisition_date, current.acquisition_date);
        diff_numeric!(acquisition_cost, current.acquisition_cost);
        diff_numeric!(current_value, current.current_value);
        diff_numeric!(depreciation_rate, current.depreciation_rate);
        diff_field!(warranty_expiry, current.warranty_expiry);
        diff_field!(metadata, current.metadata);

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
                .fetch_one(&self.pool)
                .await?;

                if child_count > 0 {
                    return Err(AppError::Conflict(format!(
                        "Cannot unset is_container: {child_count} active children exist"
                    )));
                }
            }
        }

        let event = DomainEvent::ItemUpdated(ItemUpdatedData { changes });

        let mut tx = self.pool.begin().await?;
        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
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
        let item = sqlx::query_as::<_, (Uuid, Option<Uuid>, Option<String>, String, bool)>(
            "SELECT id, parent_id, container_path::text, node_id, is_container FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(item_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        let dest = sqlx::query_as::<_, (Uuid, bool, Option<String>)>(
            "SELECT id, is_container, container_path::text FROM items WHERE id = $1 AND is_deleted = FALSE",
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

        // Circular reference check: destination must not be a descendant of the moved item
        if item.4 {
            // item is_container
            if let Some(ref item_path) = item.2 {
                if let Some(ref dest_path) = dest.2 {
                    // Proper LTREE containment: dest is self or a child of self
                    if dest_path == item_path || dest_path.starts_with(&format!("{item_path}.")) {
                        return Err(AppError::Conflict(
                            "Cannot move a container into its own descendant".into(),
                        ));
                    }
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
        // Check item exists and whether it's a container
        let row = sqlx::query_as::<_, (Uuid, bool)>(
            "SELECT id, is_container FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        // Guard: prevent deleting a non-empty container
        if row.1 {
            let child_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM items WHERE parent_id = $1 AND is_deleted = FALSE",
            )
            .bind(item_id)
            .fetch_one(&self.pool)
            .await?;

            if child_count > 0 {
                return Err(AppError::Conflict(format!(
                    "Cannot delete non-empty container ({child_count} active children). Move or delete children first."
                )));
            }
        }

        let event = DomainEvent::ItemDeleted(ItemDeletedData { reason });

        let mut tx = self.pool.begin().await?;
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
        let item = sqlx::query_as::<_, (Uuid, bool, Option<Uuid>)>(
            "SELECT id, is_deleted, parent_id FROM items WHERE id = $1",
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
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
            .fetch_one(&self.pool)
            .await?;

            if !parent_ok {
                return Err(AppError::Conflict(
                    "Cannot restore item: parent container is deleted or missing. Move to an active container first.".into(),
                ));
            }
        }

        let event = DomainEvent::ItemRestored(ItemRestoredData { from_event_id: None });

        let mut tx = self.pool.begin().await?;
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
        let event = DomainEvent::ItemImageRemoved(ItemImageRemovedData { path });

        let mut tx = self.pool.begin().await?;
        let stored = self.event_store.append_in_tx(&mut tx, item_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
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
        let event = DomainEvent::ItemExternalCodeAdded(ExternalCodeData { code_type, value });

        let mut tx = self.pool.begin().await?;
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
        let event = DomainEvent::ItemExternalCodeRemoved(ExternalCodeData { code_type, value });

        let mut tx = self.pool.begin().await?;
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
        let current = sqlx::query_as::<_, (bool, Option<i32>)>(
            "SELECT is_fungible, fungible_quantity FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        if !current.0 {
            return Err(AppError::BadRequest("Item is not fungible".into()));
        }

        let event = DomainEvent::ItemQuantityAdjusted(QuantityAdjustedData {
            old_qty: current.1,
            new_qty: req.new_quantity,
            reason: req.reason.clone(),
        });

        let mut tx = self.pool.begin().await?;
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
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let current_schema: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT location_schema FROM items WHERE id = $1 AND is_container = TRUE AND is_deleted = FALSE",
        )
        .bind(container_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Container {container_id} not found")))?;

        let event = DomainEvent::ContainerSchemaUpdated(ContainerSchemaUpdatedData {
            old_schema: current_schema,
            new_schema,
        });

        let mut tx = self.pool.begin().await?;
        let stored = self.event_store.append_in_tx(&mut tx, container_id, &event, actor_id, metadata).await?;
        Projector::apply(&mut tx, container_id, &event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }
}

/// Derive an immutable, LTREE-safe node ID from an item's UUID.
/// Produces labels like `n_4a8b3c1d2e3f` — first 12 hex chars of the UUID prefixed with `n_`.
/// 12 hex chars = 48 bits of entropy → birthday collision at ~1% requires ~16 million items,
/// well beyond household scale. LTREE labels must match `[A-Za-z_][A-Za-z0-9_]*`, so the
/// `n_` prefix ensures the label never starts with a digit. The UNIQUE constraint on
/// `node_id` catches any collision at INSERT time.
pub fn uuid_to_node_id(id: &Uuid) -> String {
    format!("n_{}", &id.simple().to_string()[..12])
}
