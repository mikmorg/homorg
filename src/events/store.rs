use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::event::{DomainEvent, EventMetadata, StoredEvent};

/// Append-only event store backed by PostgreSQL.
#[derive(Clone)]
pub struct EventStore {
    pool: PgPool,
}

impl EventStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Append a new event within the given transaction.
    /// Uses optimistic concurrency via per-aggregate sequence_number.
    /// The sequence is computed atomically in a single INSERT...SELECT to prevent races.
    pub async fn append_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        aggregate_id: Uuid,
        event: &DomainEvent,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let event_id = Uuid::new_v4();
        let event_type = event.event_type();
        let event_data = serde_json::to_value(event)
            .map_err(|e| AppError::Internal(format!("Failed to serialize event: {e}")))?;
        let meta_json = serde_json::to_value(metadata)
            .map_err(|e| AppError::Internal(format!("Failed to serialize metadata: {e}")))?;

        // Atomic sequence assignment: single INSERT...SELECT prevents race conditions.
        // The UNIQUE constraint on (aggregate_id, sequence_number) provides a safety net.
        let row = sqlx::query_as::<_, StoredEvent>(
            r#"
            INSERT INTO event_store (event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, sequence_number, schema_version)
            VALUES ($1, $2, 'item', $3, $4, $5, $6,
                    (SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM event_store WHERE aggregate_id = $2),
                    1)
            RETURNING id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
            "#,
        )
        .bind(event_id)
        .bind(aggregate_id)
        .bind(event_type)
        .bind(&event_data)
        .bind(&meta_json)
        .bind(actor_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| {
            // Surface UNIQUE violation on sequence_number as a clear conflict error
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("uq_event_store_aggregate_seq") {
                    return AppError::Conflict(
                        "Concurrent modification detected — please retry".into(),
                    );
                }
            }
            AppError::from(e)
        })?;

        Ok(row)
    }

    /// Convenience: append event outside an explicit transaction (creates its own).
    pub async fn append(
        &self,
        aggregate_id: Uuid,
        event: &DomainEvent,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<StoredEvent> {
        let mut tx = self.pool.begin().await?;
        let stored = self.append_in_tx(&mut tx, aggregate_id, event, actor_id, metadata).await?;
        tx.commit().await?;
        Ok(stored)
    }

    /// Replay all events for a given aggregate in order.
    pub async fn get_events(
        &self,
        aggregate_id: Uuid,
        from_sequence: Option<i64>,
    ) -> AppResult<Vec<StoredEvent>> {
        let from = from_sequence.unwrap_or(0);
        let rows = sqlx::query_as::<_, StoredEvent>(
            r#"
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
            FROM event_store
            WHERE aggregate_id = $1 AND sequence_number >= $2
            ORDER BY sequence_number ASC
            "#,
        )
        .bind(aggregate_id)
        .bind(from)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get events correlated by session_id.
    pub async fn get_events_by_session(&self, session_id: &str) -> AppResult<Vec<StoredEvent>> {
        let rows = sqlx::query_as::<_, StoredEvent>(
            r#"
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
            FROM event_store
            WHERE metadata->>'session_id' = $1
            ORDER BY id ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Paginated event log with optional filters.
    pub async fn get_events_paginated(
        &self,
        event_type_filter: Option<&str>,
        actor_id_filter: Option<Uuid>,
        after_id: Option<i64>,
        limit: i64,
    ) -> AppResult<Vec<StoredEvent>> {
        let after = after_id.unwrap_or(0);
        let rows = sqlx::query_as::<_, StoredEvent>(
            r#"
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
            FROM event_store
            WHERE id > $1
              AND ($2::text IS NULL OR event_type = $2)
              AND ($3::uuid IS NULL OR actor_id = $3)
            ORDER BY id ASC
            LIMIT $4
            "#,
        )
        .bind(after)
        .bind(event_type_filter)
        .bind(actor_id_filter)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Look up a single event by its event_id (UUID).
    pub async fn get_event_by_id(&self, event_id: Uuid) -> AppResult<StoredEvent> {
        sqlx::query_as::<_, StoredEvent>(
            r#"
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
            FROM event_store
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Event {event_id} not found")))
    }

    /// Look up a single event by its event_id within an existing transaction.
    pub async fn get_event_by_id_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event_id: Uuid,
    ) -> AppResult<StoredEvent> {
        sqlx::query_as::<_, StoredEvent>(
            r#"
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
            FROM event_store
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Event {event_id} not found")))
    }

    /// Get events correlated by session_id within an existing transaction.
    pub async fn get_events_by_session_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        session_id: &str,
    ) -> AppResult<Vec<StoredEvent>> {
        let rows = sqlx::query_as::<_, StoredEvent>(
            r#"
            SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
            FROM event_store
            WHERE metadata->>'session_id' = $1
            ORDER BY id ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&mut **tx)
        .await?;
        Ok(rows)
    }

    /// Check if a compensating event already exists for the given original event_id.
    /// Used for undo idempotency: prevents double-undo.
    pub async fn has_compensating_event_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        original_event_id: Uuid,
    ) -> AppResult<bool> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM event_store
                WHERE metadata->>'causation_id' = $1
            )
            "#,
        )
        .bind(original_event_id.to_string())
        .fetch_one(&mut **tx)
        .await?;
        Ok(exists)
    }
}
