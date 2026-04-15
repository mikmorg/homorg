//! Read and write helpers for the `enrichment_tasks` queue.
//!
//! Two classes of caller:
//! - the enricher daemon (`src/bin/enricher`) claims tasks and reports results
//! - admin routes list/retry/cancel tasks and list the review queue of items
//!   whose `ai_suggestions` are awaiting human decision.
//!
//! The queue uses `SELECT FOR UPDATE SKIP LOCKED` so multiple daemon instances
//! (future horizontal scaling) can safely drain in parallel; the partial
//! unique index `idx_enrichment_one_active_per_item` from migration 0025
//! keeps at most one active task per item.

use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppResult;
use crate::models::enrichment::{EnrichmentError, EnrichmentStatus, EnrichmentTask};
use crate::models::item::Item;
use crate::queries::item_queries::ITEM_FULL_SELECT;

/// Claim the highest-priority pending task atomically.
///
/// Lower `priority` values win (see migration 0025 comment), tied by
/// `created_at ASC`. Returns `Ok(None)` if the queue is empty. On success the
/// task row is flipped to `in_progress`, its `attempts` is incremented, and
/// `claimed_at`/`claimed_by` are populated. The daemon is expected to call
/// `complete_task` or `fail_task` before dropping the returned record.
///
/// `claimed_by` is typically `"<hostname>:<pid>"` so orphan detection at
/// startup can identify stuck tasks.
pub async fn claim_next_task(pool: &PgPool, claimed_by: &str) -> AppResult<Option<EnrichmentTask>> {
    let row = sqlx::query_as::<_, EnrichmentTask>(
        r#"
        UPDATE enrichment_tasks
        SET status = 'in_progress',
            claimed_at = NOW(),
            claimed_by = $1,
            attempts = attempts + 1,
            updated_at = NOW()
        WHERE id = (
            SELECT id FROM enrichment_tasks
            WHERE status = 'pending'
            ORDER BY priority ASC, created_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING
            id, item_id, trigger_event, priority, status,
            attempts, max_attempts, provider, last_error, result_summary,
            claimed_at, claimed_by, created_at, updated_at, completed_at
        "#,
    )
    .bind(claimed_by)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Mark a task as succeeded. `result_summary` is expected to include at least
/// `{confidence, model, fields_changed, cost_usd?, reasoning?}`.
pub async fn complete_task(
    pool: &PgPool,
    task_id: Uuid,
    provider: &str,
    result_summary: &serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE enrichment_tasks
        SET status         = 'succeeded',
            completed_at   = NOW(),
            updated_at     = NOW(),
            provider       = $2,
            result_summary = $3,
            last_error     = NULL
        WHERE id = $1
        "#,
    )
    .bind(task_id)
    .bind(provider)
    .bind(result_summary)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark a task as failed. Transitions to `dead` if the error is non-retryable
/// or the attempt count has reached `max_attempts`; otherwise transitions
/// back to `pending` so the next claim cycle will pick it up again.
///
/// Note: `attempts` is already incremented by `claim_next_task`, so this
/// reads the current value to decide whether the ceiling has been hit.
pub async fn fail_task(pool: &PgPool, task_id: Uuid, error: &EnrichmentError) -> AppResult<()> {
    let retryable = error.is_retryable();
    let msg = error.to_string();
    sqlx::query(
        r#"
        UPDATE enrichment_tasks
        SET status = CASE
                WHEN $2 = FALSE OR attempts >= max_attempts THEN 'dead'::enrichment_status
                ELSE 'pending'::enrichment_status
            END,
            last_error   = $3,
            claimed_at   = NULL,
            claimed_by   = NULL,
            completed_at = CASE
                WHEN $2 = FALSE OR attempts >= max_attempts THEN NOW()
                ELSE completed_at
            END,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(task_id)
    .bind(retryable)
    .bind(&msg)
    .execute(pool)
    .await?;
    Ok(())
}

/// Admin cancel: force a task to `canceled` regardless of its current state.
/// No-op if the task is already terminal. Returns true if a row was affected.
pub async fn cancel_task(pool: &PgPool, task_id: Uuid) -> AppResult<bool> {
    let res = sqlx::query(
        r#"
        UPDATE enrichment_tasks
        SET status       = 'canceled',
            completed_at = NOW(),
            updated_at   = NOW()
        WHERE id = $1
          AND status IN ('pending', 'in_progress', 'failed', 'dead')
        "#,
    )
    .bind(task_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Admin retry: push a dead/failed task back to `pending`. Clears the error
/// message but deliberately does not reset `attempts` — the counter reflects
/// real execution history and hitting `max_attempts` again should still dead-
/// letter. The admin can raise `max_attempts` first if they want more tries.
pub async fn retry_task(pool: &PgPool, task_id: Uuid) -> AppResult<bool> {
    let res = sqlx::query(
        r#"
        UPDATE enrichment_tasks
        SET status       = 'pending',
            last_error   = NULL,
            claimed_at   = NULL,
            claimed_by   = NULL,
            completed_at = NULL,
            updated_at   = NOW()
        WHERE id = $1
          AND status IN ('failed', 'dead', 'canceled')
        "#,
    )
    .bind(task_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Fetch a single task row (admin detail view).
pub async fn get_task(pool: &PgPool, task_id: Uuid) -> AppResult<Option<EnrichmentTask>> {
    let row = sqlx::query_as::<_, EnrichmentTask>(
        r#"
        SELECT id, item_id, trigger_event, priority, status,
               attempts, max_attempts, provider, last_error, result_summary,
               claimed_at, claimed_by, created_at, updated_at, completed_at
        FROM enrichment_tasks
        WHERE id = $1
        "#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Paginated task list. When `status_filter` is None the full queue is
/// returned, newest first. Caller is responsible for clamping `limit`.
pub async fn list_tasks(
    pool: &PgPool,
    status_filter: Option<EnrichmentStatus>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<EnrichmentTask>> {
    let rows = sqlx::query_as::<_, EnrichmentTask>(
        r#"
        SELECT id, item_id, trigger_event, priority, status,
               attempts, max_attempts, provider, last_error, result_summary,
               claimed_at, claimed_by, created_at, updated_at, completed_at
        FROM enrichment_tasks
        WHERE $1::enrichment_status IS NULL OR status = $1
        ORDER BY created_at DESC
        LIMIT $2
        OFFSET $3
        "#,
    )
    .bind(status_filter)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Items in the review queue: `ai_suggestions` populated, `needs_review=TRUE`,
/// not deleted. Ordered by confidence ascending (worst first — admins should
/// see the least-confident items first) with `NULLS FIRST`, tied by oldest
/// `updated_at` so stale review items don't live forever at the bottom.
pub async fn list_review_queue(pool: &PgPool, limit: i64, offset: i64) -> AppResult<Vec<Item>> {
    let sql = format!(
        r#"
        SELECT {ITEM_FULL_SELECT}
        WHERE i.ai_suggestions IS NOT NULL
          AND i.needs_review = TRUE
          AND i.is_deleted = FALSE
        ORDER BY i.classification_confidence ASC NULLS FIRST, i.updated_at ASC
        LIMIT $1
        OFFSET $2
        "#
    );
    let rows = sqlx::query_as::<_, Item>(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// Count of items awaiting review — used to render the admin nav badge.
pub async fn count_review_queue(pool: &PgPool) -> AppResult<i64> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM items
        WHERE ai_suggestions IS NOT NULL
          AND needs_review = TRUE
          AND is_deleted = FALSE
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(n)
}
