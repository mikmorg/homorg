use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::events::projector::Projector;
use crate::events::store::EventStore;
use crate::models::event::*;

/// Command handler for undo operations.
#[derive(Clone)]
pub struct UndoCommands {
    pool: PgPool,
    event_store: EventStore,
}

impl UndoCommands {
    pub fn new(pool: PgPool, event_store: EventStore) -> Self {
        Self { pool, event_store }
    }

    /// Undo a single event by generating a compensating event.
    /// Any member can undo any event (no item ownership model).
    /// Idempotent: returns Conflict if already undone.
    pub async fn undo_event(
        &self,
        event_id: Uuid,
        actor_id: Uuid,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;

        // Idempotency guard: reject if already undone
        if self.event_store.has_compensating_event_in_tx(&mut tx, event_id).await? {
            return Err(AppError::Conflict(format!("Event {event_id} has already been undone")));
        }

        let original = self.event_store.get_event_by_id_in_tx(&mut tx, event_id).await?;
        let domain_event: DomainEvent = serde_json::from_value(original.event_data.clone())
            .map_err(|e| AppError::Internal(format!("Failed to deserialize event: {e}")))?;

        let (compensating_event, aggregate_id) = self.build_compensating_event(
            &mut tx,
            &domain_event,
            original.aggregate_id,
            original.event_id,
        ).await?;

        let metadata = EventMetadata {
            causation_id: Some(event_id.to_string()),
            ..Default::default()
        };

        let stored = self.event_store.append_in_tx(
            &mut tx, aggregate_id, &compensating_event, actor_id, &metadata,
        ).await?;
        Projector::apply(&mut tx, aggregate_id, &compensating_event, actor_id).await?;
        tx.commit().await?;

        Ok(stored)
    }

    /// Undo a batch of events in reverse chronological order.
    /// All compensating events are applied atomically in a single transaction.
    /// Any member can undo any event (no item ownership model).
    pub async fn undo_batch(
        &self,
        event_ids: &[Uuid],
        actor_id: Uuid,
    ) -> AppResult<Vec<StoredEvent>> {
        let mut tx = self.pool.begin().await?;
        let mut results = Vec::new();

        // Process in reverse order (most recent first) for consistency
        for &eid in event_ids.iter().rev() {
            // Idempotency guard: skip events already undone
            if self.event_store.has_compensating_event_in_tx(&mut tx, eid).await? {
                continue;
            }

            let original = self.event_store.get_event_by_id_in_tx(&mut tx, eid).await?;
            let domain_event: DomainEvent = serde_json::from_value(original.event_data.clone())
                .map_err(|e| AppError::Internal(format!("Failed to deserialize event: {e}")))?;

            let (compensating_event, aggregate_id) = self.build_compensating_event(
                &mut tx,
                &domain_event,
                original.aggregate_id,
                original.event_id,
            ).await?;

            let metadata = EventMetadata {
                causation_id: Some(eid.to_string()),
                ..Default::default()
            };

            let stored = self.event_store.append_in_tx(
                &mut tx, aggregate_id, &compensating_event, actor_id, &metadata,
            ).await?;
            Projector::apply(&mut tx, aggregate_id, &compensating_event, actor_id).await?;
            results.push(stored);
        }

        tx.commit().await?;
        Ok(results)
    }

    /// Undo all events from a stocker session.
    /// Reads session events and generates compensating events in a single transaction (no TOCTOU).
    /// Any member can undo any session (no item ownership model).
    ///
    /// `max_events` caps the number of events that can be undone in a single session call to
    /// prevent unexpectedly large transactions (DoS-1).  Pass `config.max_batch_size`.
    pub async fn undo_session(
        &self,
        session_id: &str,
        actor_id: Uuid,
        max_events: usize,
    ) -> AppResult<Vec<StoredEvent>> {
        let mut tx = self.pool.begin().await?;
        let events = self.event_store.get_events_by_session_in_tx(&mut tx, session_id).await?;

        // DoS-1: Reject sessions with more events than the configured batch limit so a
        // long-running session cannot trigger an unbounded single transaction.
        if events.len() > max_events {
            return Err(AppError::BadRequest(format!(
                "Session has {} events; maximum undo batch is {max_events}. \
                 Use event_ids for a partial undo.",
                events.len()
            )));
        }

        let event_ids: Vec<Uuid> = events.iter().map(|e| e.event_id).collect();

        let mut results = Vec::new();

        // Process in reverse order within the same transaction
        for &eid in event_ids.iter().rev() {
            // Idempotency guard: skip events already undone
            if self.event_store.has_compensating_event_in_tx(&mut tx, eid).await? {
                continue;
            }

            let original = self.event_store.get_event_by_id_in_tx(&mut tx, eid).await?;
            let domain_event: DomainEvent = serde_json::from_value(original.event_data.clone())
                .map_err(|e| AppError::Internal(format!("Failed to deserialize event: {e}")))?;

            let (compensating_event, aggregate_id) = self.build_compensating_event(
                &mut tx,
                &domain_event,
                original.aggregate_id,
                original.event_id,
            ).await?;

            let metadata = EventMetadata {
                causation_id: Some(eid.to_string()),
                ..Default::default()
            };

            let stored = self.event_store.append_in_tx(
                &mut tx, aggregate_id, &compensating_event, actor_id, &metadata,
            ).await?;
            Projector::apply(&mut tx, aggregate_id, &compensating_event, actor_id).await?;
            results.push(stored);
        }

        tx.commit().await?;
        Ok(results)
    }

    async fn build_compensating_event(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        original: &DomainEvent,
        aggregate_id: Uuid,
        original_event_id: Uuid,
    ) -> AppResult<(DomainEvent, Uuid)> {
        match original {
            DomainEvent::ItemMoved(data) => {
                // Reverse the move: go back to original container
                let compensating = DomainEvent::ItemMoveReverted(ItemMoveRevertedData {
                    original_event_id,
                    from_container_id: data.to_container_id,
                    to_container_id: data.from_container_id,
                    from_path: data.to_path.clone(),
                    to_path: data.from_path.clone(),
                    coordinate: None,
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ItemCreated(_) => {
                // DI-1: Guard — cannot undo creation if item has active children
                let child_count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM items WHERE parent_id = $1 AND is_deleted = FALSE",
                )
                .bind(aggregate_id)
                .fetch_one(&mut **tx)
                .await?;

                if child_count > 0 {
                    return Err(AppError::Conflict(format!(
                        "Cannot undo ItemCreated: item has {child_count} active children. \
                         Move or delete them first."
                    )));
                }

                let compensating = DomainEvent::ItemDeleted(ItemDeletedData {
                    reason: Some(format!("Undo of creation event {original_event_id}")),
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ItemDeleted(_) => {
                let compensating = DomainEvent::ItemRestored(ItemRestoredData {
                    from_event_id: Some(original_event_id),
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ItemUpdated(data) => {
                let reversed_changes: Vec<FieldChange> = data.changes.iter().map(|c| FieldChange {
                    field: c.field.clone(),
                    old: c.new.clone(),
                    new: c.old.clone(),
                }).collect();
                let compensating = DomainEvent::ItemUpdated(ItemUpdatedData {
                    changes: reversed_changes,
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ItemQuantityAdjusted(data) => {
                let compensating = DomainEvent::ItemQuantityAdjusted(QuantityAdjustedData {
                    old_qty: Some(data.new_qty),
                    new_qty: data.old_qty.unwrap_or(0),
                    reason: Some(format!("Undo of event {original_event_id}")),
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ItemExternalCodeAdded(data) => {
                let compensating = DomainEvent::ItemExternalCodeRemoved(ExternalCodeData {
                    code_type: data.code_type.clone(),
                    value: data.value.clone(),
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ItemExternalCodeRemoved(data) => {
                let compensating = DomainEvent::ItemExternalCodeAdded(ExternalCodeData {
                    code_type: data.code_type.clone(),
                    value: data.value.clone(),
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ItemImageAdded(data) => {
                // When undoing an ItemImageAdded, preserve caption/order so further undo is lossy-free
                let compensating = DomainEvent::ItemImageRemoved(ItemImageRemovedData {
                    path: data.path.clone(),
                    caption: data.caption.clone(),
                    order: Some(data.order),
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ItemImageRemoved(data) => {
                // ES-3: restore with preserved caption/order from the removal event
                let compensating = DomainEvent::ItemImageAdded(ItemImageAddedData {
                    path: data.path.clone(),
                    caption: data.caption.clone(),
                    order: data.order.unwrap_or(0),
                });
                Ok((compensating, aggregate_id))
            }
            DomainEvent::ContainerSchemaUpdated(data) => {
                // ES-1: Swap old↔new schema to restore the previous schema
                let compensating = DomainEvent::ContainerSchemaUpdated(ContainerSchemaUpdatedData {
                    old_schema: Some(data.new_schema.clone()),
                    new_schema: data.old_schema.clone().unwrap_or(serde_json::Value::Null),
                });
                Ok((compensating, aggregate_id))
            }
            _ => Err(AppError::BadRequest(format!(
                "Cannot undo event type: {}", original.event_type()
            ))),
        }
    }
}
