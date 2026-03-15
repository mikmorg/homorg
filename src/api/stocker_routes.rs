use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::errors::{AppError, AppResult};
use crate::models::event::EventMetadata;
use crate::models::item::{CreateItemRequest, MoveItemRequest};
use crate::models::session::*;
use crate::api::item_routes::validate_create_request;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions", post(start_session).get(list_sessions))
        .route(
            "/sessions/{id}",
            get(get_session),
        )
        .route("/sessions/{id}/batch", post(submit_batch))
        .route("/sessions/{id}/end", put(end_session))
}

// Field limits matching DB schema column widths for scan sessions.
const MAX_SESSION_DEVICE_ID_LEN: usize = 128;
const MAX_SESSION_NOTES_BYTES: usize = 10_000;

/// Start a new scan session.
async fn start_session(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    body: Option<Json<StartSessionRequest>>,
) -> AppResult<(StatusCode, Json<ScanSession>)> {
    auth.require_role("member")?;
    let req = body.map(|b| b.0).unwrap_or_default();

    // Resolve initial container barcode if provided.
    let initial_container_id = if let Some(ref barcode) = req.initial_container_barcode {
        let container = sqlx::query_as::<_, (Uuid, bool)>(
            "SELECT id, is_container FROM items WHERE system_barcode = $1 AND is_deleted = FALSE",
        )
        .bind(barcode)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Container barcode {barcode} not found")))?;

        if !container.1 {
            return Err(AppError::BadRequest(format!(
                "Barcode {barcode} is not a container"
            )));
        }
        Some(container.0)
    } else {
        None
    };

    let session_id = Uuid::new_v4();

    // VAL: Validate VARCHAR(128) device_id and unbounded TEXT notes before hitting the DB.
    if let Some(ref did) = req.device_id {
        if did.chars().count() > MAX_SESSION_DEVICE_ID_LEN {
            return Err(AppError::BadRequest(format!(
                "device_id exceeds maximum length of {MAX_SESSION_DEVICE_ID_LEN} characters"
            )));
        }
    }
    if let Some(ref notes) = req.notes {
        if notes.len() > MAX_SESSION_NOTES_BYTES {
            return Err(AppError::BadRequest(format!(
                "notes exceeds maximum size of {MAX_SESSION_NOTES_BYTES} bytes"
            )));
        }
    }

    let session = state.session_repository.create(
        session_id,
        auth.user_id,
        req.device_id.as_deref(),
        req.notes.as_deref(),
        initial_container_id,
    ).await?;
    Ok((StatusCode::CREATED, Json(session)))
}

#[derive(Debug, Deserialize)]
struct ListSessionsQuery {
    limit: Option<i64>,
}

/// List the current user's scan sessions.
async fn list_sessions(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(q): Query<ListSessionsQuery>,
) -> AppResult<Json<Vec<ScanSession>>> {
    let limit = q.limit.unwrap_or(20).min(100);
    let sessions = state.session_repository.list_for_user(auth.user_id, limit).await?;
    Ok(Json(sessions))
}

/// Get a single session detail.
async fn get_session(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ScanSession>> {
    let session = state.session_repository.get_for_user(id, auth.user_id).await?;
    Ok(Json(session))
}

/// Submit a batch of scan events within a session.
async fn submit_batch(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
    Query(params): Query<BatchQueryParams>,
    Json(req): Json<StockerBatchRequest>,
) -> AppResult<Json<StockerBatchResponse>> {
    auth.require_role("member")?;
    // Validate session exists and belongs to user
    let session = state.session_repository.get_active_for_user(session_id, auth.user_id).await?;

    if req.events.len() > state.config.max_batch_size {
        return Err(AppError::BadRequest(format!(
            "Batch size {} exceeds maximum {}",
            req.events.len(),
            state.config.max_batch_size
        )));
    }

    let atomic = params.atomic.unwrap_or(false);
    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut active_container_id = session.active_container_id;
    let mut items_scanned: i32 = 0;
    let mut items_created: i32 = 0;
    let mut items_moved: i32 = 0;
    let mut items_errored: i32 = 0;

    if atomic {
        // True atomic mode: wrap all operations in a single transaction.
        // Any failure rolls back the entire batch.
        let mut tx = state.pool.begin().await?;

        for (index, batch_event) in req.events.iter().enumerate() {
            let result = process_batch_event_in_tx(
                &state,
                &mut tx,
                auth.user_id,
                session_id,
                batch_event,
                &mut active_container_id,
                index,
            )
            .await?; // ? propagates error, rolling back tx on drop

            // CB-6: Count inline per result type rather than a fragile +1/-1 dance.
            match &result {
                StockerBatchResult::Created { .. } => { items_scanned += 1; items_created += 1; }
                StockerBatchResult::Moved { .. } => { items_scanned += 1; items_moved += 1; }
                StockerBatchResult::ContextSet { .. } | StockerBatchResult::Resolved { .. } => {} // not a physical scan
            }
            results.push(result);
        }

        // Update session stats within the same transaction
        state.session_repository.update_stats_in_tx(
            &mut tx, session_id, active_container_id, items_scanned, items_created, items_moved, items_errored,
        ).await?;

        tx.commit().await?;
    } else {
        // Best-effort mode: each event commits independently, errors are collected
        for (index, batch_event) in req.events.iter().enumerate() {
            let result = process_batch_event(
                &state,
                auth.user_id,
                session_id,
                batch_event,
                &mut active_container_id,
                index,
            )
            .await;

            match result {
                Ok(batch_result) => {
                    // CB-6: Count inline per result type.
                    match &batch_result {
                        StockerBatchResult::Created { .. } => { items_scanned += 1; items_created += 1; }
                        StockerBatchResult::Moved { .. } => { items_scanned += 1; items_moved += 1; }
                        StockerBatchResult::ContextSet { .. } | StockerBatchResult::Resolved { .. } => {}
                    }
                    results.push(batch_result);
                }
                Err(e) => {
                    items_errored += 1;
                    errors.push(StockerBatchError {
                        index,
                        code: e.error_code().to_string(),
                        message: e.to_string(),
                    });
                }
            }
        }

        // Update session stats
        state.session_repository.update_stats(
            session_id, active_container_id, items_scanned, items_created, items_moved, items_errored,
        ).await?;
    }

    Ok(Json(StockerBatchResponse {
        processed: results.len(),
        results,
        errors,
    }))
}

#[derive(Debug, Deserialize)]
struct BatchQueryParams {
    atomic: Option<bool>,
}

/// Process a single batch event (best-effort mode).
/// Wraps a mini-transaction around `process_batch_event_in_tx`.
async fn process_batch_event(
    state: &Arc<AppState>,
    actor_id: Uuid,
    session_id: Uuid,
    event: &StockerBatchEvent,
    active_container_id: &mut Option<Uuid>,
    index: usize,
) -> AppResult<StockerBatchResult> {
    let mut tx = state.pool.begin().await?;
    let result = process_batch_event_in_tx(
        state,
        &mut tx,
        actor_id,
        session_id,
        event,
        active_container_id,
        index,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

/// Process a single batch event within an external transaction (for atomic mode).
async fn process_batch_event_in_tx(
    state: &Arc<AppState>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor_id: Uuid,
    session_id: Uuid,
    event: &StockerBatchEvent,
    active_container_id: &mut Option<Uuid>,
    index: usize,
) -> AppResult<StockerBatchResult> {
    // H-2: Build metadata here so we can capture client-side scanned_at per event.
    let scanned_at_str = match event {
        StockerBatchEvent::SetContext { scanned_at, .. }
        | StockerBatchEvent::MoveItem { scanned_at, .. }
        | StockerBatchEvent::CreateAndPlace { scanned_at, .. }
        | StockerBatchEvent::Resolve { scanned_at, .. } => Some(scanned_at.to_rfc3339()),
    };
    let metadata = EventMetadata {
        session_id: Some(session_id.to_string()),
        scanned_at: scanned_at_str,
        ..Default::default()
    };
    match event {
        StockerBatchEvent::SetContext { barcode, .. } => {
            let container = sqlx::query_as::<_, (Uuid, bool)>(
                "SELECT id, is_container FROM items WHERE system_barcode = $1 AND is_deleted = FALSE",
            )
            .bind(barcode)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Container barcode {barcode} not found")))?;

            if !container.1 {
                return Err(AppError::BadRequest(format!(
                    "Barcode {barcode} is not a container"
                )));
            }

            *active_container_id = Some(container.0);

            Ok(StockerBatchResult::ContextSet {
                index,
                status: "ok".into(),
                context_set: barcode.clone(),
            })
        }
        StockerBatchEvent::MoveItem {
            barcode,
            coordinate,
            ..
        } => {
            let container_id = active_container_id.ok_or_else(|| {
                AppError::BadRequest("No active container set. Send set_context first.".into())
            })?;

            let item_id: Uuid = sqlx::query_scalar(
                "SELECT id FROM items WHERE system_barcode = $1 AND is_deleted = FALSE",
            )
            .bind(barcode)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item {barcode} not found")))?;

            let move_req = MoveItemRequest {
                container_id,
                coordinate: coordinate.clone(),
            };

            let stored = state
                .item_commands
                .move_item_in_tx(tx, item_id, &move_req, actor_id, &metadata)
                .await?;

            Ok(StockerBatchResult::Moved {
                index,
                status: "ok".into(),
                event_id: stored.event_id,
            })
        }
        StockerBatchEvent::CreateAndPlace {
            barcode,
            name,
            description,
            category,
            tags,
            is_container,
            coordinate,
            condition,
            metadata: item_metadata,
            is_fungible,
            fungible_quantity,
            fungible_unit,
            external_codes,
            container_type_id,
            ..
        } => {
            let container_id = active_container_id.ok_or_else(|| {
                AppError::BadRequest("No active container set. Send set_context first.".into())
            })?;

            let item_id = Uuid::new_v4();

            // Barcodes are optional. A non-empty scanned barcode string is used directly;
            // empty means the item has no barcode yet (can be assigned later via
            // POST /items/{id}/barcode).
            let system_barcode: Option<String> = if barcode.is_empty() {
                None
            } else {
                Some(barcode.clone())
            };

            let create_req = CreateItemRequest {
                system_barcode,
                parent_id: container_id,
                name: name.clone(),
                description: description.clone(),
                category: category.clone(),
                tags: tags.clone(),
                is_container: *is_container,
                coordinate: coordinate.clone(),
                location_schema: None,
                max_capacity_cc: None,
                max_weight_grams: None,
                dimensions: None,
                weight_grams: None,
                is_fungible: *is_fungible,
                fungible_quantity: *fungible_quantity,
                fungible_unit: fungible_unit.clone(),
                external_codes: external_codes.clone(),
                condition: condition.clone(),
                acquisition_date: None,
                acquisition_cost: None,
                current_value: None,
                depreciation_rate: None,
                warranty_expiry: None,
                metadata: item_metadata.clone(),
                container_type_id: *container_type_id,
            };

            // API-2: Apply the same field-length validation used by the items API.
            validate_create_request(&create_req)?;

            let stored = state
                .item_commands
                .create_item_in_tx(tx, item_id, &create_req, actor_id, &metadata)
                .await?;

            let needs_details = name.is_none() || name.as_deref().is_none_or(|n| n.is_empty());

            Ok(StockerBatchResult::Created {
                index,
                status: "ok".into(),
                event_id: stored.event_id,
                item_id,
                needs_details,
            })
        }
        StockerBatchEvent::Resolve { barcode, .. } => {
            // M-3: Use resolve_barcode_in_tx so that items created earlier in the
            // same atomic batch are visible (they exist only inside the open tx).
            let resolution = state.barcode_commands.resolve_barcode_in_tx(tx, barcode).await?;
            Ok(StockerBatchResult::Resolved {
                index,
                status: "ok".into(),
                resolution,
            })
        }
    }
}

/// End a scan session.
async fn end_session(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ScanSession>> {
    auth.require_role("member")?;
    let session = state.session_repository.end_session(id, auth.user_id).await?;
    Ok(Json(session))
}
