//! Admin-gated routes for the AI enrichment system.
//!
//! Two surfaces:
//! - **Review queue** (`/admin/enrichment/review`, plus per-item approve / reject
//!   / rerun) — items whose `ai_suggestions` await a human decision.
//! - **Task monitor** (`/admin/enrichment/tasks`, plus per-task retry / cancel)
//!   — the raw `enrichment_tasks` queue.
//!
//! Every handler `auth.require_role("admin")?`s before touching state.
//!
//! Approval path: load the stashed [`AiSuggestions`], convert to an
//! [`EnrichmentOutput`], run it through [`diff_fields`] against the current
//! item state, append a single `ItemUpdated` event with the changes plus the
//! `ai_suggestions=null` / `needs_review=false` / `classification_confidence`
//! bookkeeping. The event is authored by the admin (not AI_ENRICHER_USER_ID)
//! so undo / history attribute the action to the human who approved.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::enrichment::dispatch::{diff_fields, CurrentFields};
use crate::errors::{AppError, AppResult};
use crate::events::projector::Projector;
use crate::models::enrichment::{AiSuggestions, EnrichmentOutput, EnrichmentStatus, EnrichmentTask, EnrichmentTrigger};
use crate::models::event::{DomainEvent, EventMetadata, FieldChange, ItemUpdatedData};
use crate::models::item::Item;
use crate::queries::enrichment_queries::{cancel_task, count_review_queue, list_review_queue, list_tasks, retry_task};
use crate::queries::item_queries::ITEM_FULL_SELECT;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin/enrichment/review", get(list_review_handler))
        .route("/admin/enrichment/review/count", get(count_review_handler))
        .route("/admin/enrichment/items/{id}/approve", post(approve_handler))
        .route("/admin/enrichment/items/{id}/reject", post(reject_handler))
        .route("/admin/enrichment/items/{id}/rerun", post(rerun_handler))
        .route("/admin/enrichment/tasks", get(list_tasks_handler))
        .route("/admin/enrichment/tasks/{id}/retry", post(retry_handler))
        .route("/admin/enrichment/tasks/{id}/cancel", post(cancel_handler))
}

// ── Request / response shapes ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PageParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Deserialize)]
pub struct TaskListParams {
    pub status: Option<EnrichmentStatus>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct ReviewListResponse {
    pub items: Vec<Item>,
    pub total: i64,
}

/// Per-field accept map — when provided, only fields set to `true` are applied.
/// When absent or empty, all suggested fields are applied.
#[derive(Debug, Default, Deserialize)]
pub struct ApproveRequest {
    #[serde(default)]
    pub accept: HashMap<String, bool>,
}

#[derive(Debug, Serialize)]
pub struct RerunResponse {
    pub task_id: Uuid,
}

// ── Handlers: review queue ───────────────────────────────────────────────────

async fn list_review_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<PageParams>,
) -> AppResult<Json<ReviewListResponse>> {
    auth.require_role("admin")?;
    let limit = params.limit.clamp(1, 200);
    let offset = params.offset.max(0);
    let items = list_review_queue(&state.pool, limit, offset).await?;
    let total = count_review_queue(&state.pool).await?;
    Ok(Json(ReviewListResponse { items, total }))
}

async fn count_review_handler(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<Json<Value>> {
    auth.require_role("admin")?;
    let total = count_review_queue(&state.pool).await?;
    Ok(Json(json!({ "total": total })))
}

async fn approve_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(item_id): Path<Uuid>,
    body: Option<Json<ApproveRequest>>,
) -> AppResult<Json<Value>> {
    auth.require_role("admin")?;
    let req = body.map(|Json(b)| b).unwrap_or_default();

    let item = fetch_item(&state.pool, item_id).await?;
    let suggestions_json = item
        .ai_suggestions
        .clone()
        .ok_or_else(|| AppError::NotFound(format!("No pending suggestions for item {item_id}")))?;
    let suggestions: AiSuggestions = serde_json::from_value(suggestions_json)
        .map_err(|e| AppError::Internal(format!("deserialize ai_suggestions: {e}")))?;

    let mut output = suggestions_to_output(&suggestions);
    apply_accept_filter(&mut output, &req.accept);

    let current = current_fields_from_item(&item);
    let mut changes = diff_fields(&current, &output);
    push_clear_suggestions(&mut changes, &current, output.confidence);

    let metadata = EventMetadata {
        ai_model: Some(suggestions.model.clone()),
        ai_task_id: Some(suggestions.task_id),
        ..Default::default()
    };
    append_item_updated_tx(&state, item_id, changes, auth.user_id, metadata).await?;

    Ok(Json(json!({
        "item_id": item_id,
        "applied_fields": accepted_field_names(&req.accept, &output),
    })))
}

async fn reject_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(item_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    auth.require_role("admin")?;
    let item = fetch_item(&state.pool, item_id).await?;
    let suggestions_json = item
        .ai_suggestions
        .clone()
        .ok_or_else(|| AppError::NotFound(format!("No pending suggestions for item {item_id}")))?;
    // Deserialize only so we can carry the task_id into the audit metadata; if
    // the blob is malformed we still want reject to succeed (admin escape
    // hatch), so we log and fall back to empty metadata.
    let (ai_model, ai_task_id) = match serde_json::from_value::<AiSuggestions>(suggestions_json) {
        Ok(s) => (Some(s.model), Some(s.task_id)),
        Err(e) => {
            tracing::warn!(%item_id, error = %e, "reject: ai_suggestions unparseable — proceeding without audit ids");
            (None, None)
        }
    };

    let changes = vec![
        FieldChange {
            field: "ai_suggestions".into(),
            old: item.ai_suggestions.clone().unwrap_or(Value::Null),
            new: Value::Null,
        },
        FieldChange {
            field: "needs_review".into(),
            old: Value::Bool(item.needs_review),
            new: Value::Bool(false),
        },
    ];

    let metadata = EventMetadata {
        ai_model,
        ai_task_id,
        ..Default::default()
    };
    append_item_updated_tx(&state, item_id, changes, auth.user_id, metadata).await?;

    Ok(Json(json!({ "item_id": item_id, "rejected": true })))
}

async fn rerun_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(item_id): Path<Uuid>,
) -> AppResult<Json<RerunResponse>> {
    auth.require_role("admin")?;

    // Confirm the item exists and isn't soft-deleted before enqueuing.
    let _ = fetch_item(&state.pool, item_id).await?;

    let trigger = EnrichmentTrigger::ManualRerun;
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO enrichment_tasks (item_id, trigger_event, priority)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
    )
    .bind(item_id)
    .bind(trigger.as_str())
    .bind(trigger.default_priority())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        // Partial unique index "idx_enrichment_one_active_per_item" on
        // (item_id) WHERE status IN ('pending','in_progress'). If it blocks
        // the insert, return 409 Conflict rather than 500.
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.constraint() == Some("idx_enrichment_one_active_per_item") {
                return AppError::Conflict(format!(
                    "An enrichment task is already pending or in progress for item {item_id}"
                ));
            }
        }
        AppError::Database(e)
    })?;

    Ok(Json(RerunResponse { task_id: row.0 }))
}

// ── Handlers: task queue ─────────────────────────────────────────────────────

async fn list_tasks_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<TaskListParams>,
) -> AppResult<Json<Vec<EnrichmentTask>>> {
    auth.require_role("admin")?;
    let limit = params.limit.clamp(1, 500);
    let offset = params.offset.max(0);
    let rows = list_tasks(&state.pool, params.status, limit, offset).await?;
    Ok(Json(rows))
}

async fn retry_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    auth.require_role("admin")?;
    let ok = retry_task(&state.pool, task_id).await?;
    if !ok {
        return Err(AppError::Conflict(format!(
            "Task {task_id} not in a retryable state (failed/dead/canceled)"
        )));
    }
    Ok(Json(json!({ "task_id": task_id, "retried": true })))
}

async fn cancel_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    auth.require_role("admin")?;
    let ok = cancel_task(&state.pool, task_id).await?;
    if !ok {
        return Err(AppError::Conflict(format!("Task {task_id} not in a cancelable state")));
    }
    Ok(Json(json!({ "task_id": task_id, "canceled": true })))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn fetch_item(pool: &sqlx::PgPool, item_id: Uuid) -> AppResult<Item> {
    let sql = format!("SELECT {ITEM_FULL_SELECT} WHERE i.id = $1 AND i.is_deleted = FALSE");
    sqlx::query_as::<_, Item>(&sql)
        .bind(item_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))
}

fn current_fields_from_item(item: &Item) -> CurrentFields {
    CurrentFields {
        name: item.name.clone(),
        description: item.description.clone(),
        category: item.category.clone(),
        tags: item.tags.clone(),
        metadata: item.metadata.clone(),
        classification_confidence: item.classification_confidence,
        needs_review: item.needs_review,
    }
}

fn suggestions_to_output(s: &AiSuggestions) -> EnrichmentOutput {
    EnrichmentOutput {
        name: s.name.clone(),
        description: s.description.clone(),
        tags: s.tags.clone(),
        category: s.category.clone(),
        metadata_additions: s.metadata_additions.clone(),
        discovered_codes: s.discovered_codes.clone(),
        confidence: s.confidence,
        reasoning: s.reasoning.clone(),
    }
}

/// When the body includes an `accept` map, zero out any field the admin
/// didn't check. `diff_fields` treats `None` / empty-vec / null-metadata as
/// "no change", so this is a clean way to scope the approval.
fn apply_accept_filter(output: &mut EnrichmentOutput, accept: &HashMap<String, bool>) {
    if accept.is_empty() {
        return;
    }
    let on = |k: &str| accept.get(k).copied().unwrap_or(false);
    if !on("name") {
        output.name = None;
    }
    if !on("description") {
        output.description = None;
    }
    if !on("category") {
        output.category = None;
    }
    if !on("tags") {
        output.tags.clear();
    }
    if !on("metadata") {
        output.metadata_additions = Value::Null;
    }
}

fn push_clear_suggestions(changes: &mut Vec<FieldChange>, current: &CurrentFields, confidence: f32) {
    changes.push(FieldChange {
        field: "ai_suggestions".into(),
        old: Value::Null,
        new: Value::Null,
    });
    changes.push(FieldChange {
        field: "needs_review".into(),
        old: Value::Bool(current.needs_review),
        new: Value::Bool(false),
    });
    changes.push(FieldChange {
        field: "classification_confidence".into(),
        old: current
            .classification_confidence
            .map(|f| Value::from(f as f64))
            .unwrap_or(Value::Null),
        new: Value::from(confidence as f64),
    });
}

fn accepted_field_names(accept: &HashMap<String, bool>, output: &EnrichmentOutput) -> Vec<String> {
    let mut out = Vec::new();
    let include = |k: &str| accept.is_empty() || accept.get(k).copied().unwrap_or(false);
    if include("name") && output.name.is_some() {
        out.push("name".into());
    }
    if include("description") && output.description.is_some() {
        out.push("description".into());
    }
    if include("category") && output.category.is_some() {
        out.push("category".into());
    }
    if include("tags") && !output.tags.is_empty() {
        out.push("tags".into());
    }
    if include("metadata") && !output.metadata_additions.is_null() {
        out.push("metadata".into());
    }
    out
}

async fn append_item_updated_tx(
    state: &Arc<AppState>,
    item_id: Uuid,
    changes: Vec<FieldChange>,
    actor_id: Uuid,
    metadata: EventMetadata,
) -> AppResult<()> {
    let event = DomainEvent::ItemUpdated(ItemUpdatedData { changes });
    let mut tx = state.pool.begin().await?;
    state
        .event_store
        .append_in_tx(&mut tx, item_id, &event, actor_id, &metadata)
        .await?;
    Projector::apply(&mut tx, item_id, &event, actor_id).await?;
    state.event_store.commit_and_notify(tx).await?;
    Ok(())
}
