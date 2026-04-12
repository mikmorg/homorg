use axum::{
    extract::{Json, Multipart, Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, post, put},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::item_routes::validate_create_request;
use crate::auth::middleware::AuthUser;
use crate::errors::{AppError, AppResult};
use crate::models::camera::*;
use crate::models::event::EventMetadata;
use crate::models::item::{CreateItemRequest, MoveItemRequest};
use crate::models::session::*;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions", post(start_session).get(list_sessions))
        .route(
            "/sessions/{id}",
            get(get_session),
        )
        .route("/sessions/{id}/batch", post(submit_batch))
        .route("/sessions/{id}/stream", get(session_event_stream))
        .route("/sessions/{id}/end", put(end_session))
        // Camera link management (JWT-authenticated)
        .route("/sessions/{id}/camera-links", post(create_camera_link).get(list_camera_links))
        .route("/sessions/{id}/camera-links/{token_id}", delete(revoke_camera_link))
        // Camera device endpoints (token-authenticated, no JWT required)
        .route("/camera/{token}/status", get(camera_status))
        .route("/camera/{token}/upload", post(camera_upload))
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

    // Validate initial container if provided.
    let initial_container_id = if let Some(container_id) = req.initial_container_id {
        let is_container: Option<bool> =
            sqlx::query_scalar("SELECT is_container FROM items WHERE id = $1 AND is_deleted = FALSE")
                .bind(container_id)
                .fetch_optional(&state.pool)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Container {container_id} not found")))?;

        if !is_container.unwrap_or(false) {
            return Err(AppError::BadRequest(format!("Item {container_id} is not a container")));
        }
        Some(container_id)
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

    let session = state
        .session_repository
        .create(
            session_id,
            auth.user_id,
            req.device_id.as_deref(),
            req.notes.as_deref(),
            initial_container_id,
        )
        .await?;
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
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
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
    let session = state
        .session_repository
        .get_active_for_user(session_id, auth.user_id)
        .await?;

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
    let mut active_item_id = session.active_item_id;
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
                &mut active_item_id,
                index,
            )
            .await?; // ? propagates error, rolling back tx on drop

            // CB-6: Count inline per result type rather than a fragile +1/-1 dance.
            match &result {
                StockerBatchResult::Created { .. } => {
                    items_scanned += 1;
                    items_created += 1;
                }
                StockerBatchResult::Moved { .. } => {
                    items_scanned += 1;
                    items_moved += 1;
                }
                StockerBatchResult::ContextSet { .. } | StockerBatchResult::Resolved { .. } => {} // not a physical scan
            }
            results.push(result);
        }

        // Update session stats within the same transaction
        state
            .session_repository
            .update_stats_in_tx(
                &mut tx,
                session_id,
                active_container_id,
                active_item_id,
                items_scanned,
                items_created,
                items_moved,
                items_errored,
            )
            .await?;

        state.event_store.commit_and_notify(tx).await?;
    } else {
        // Best-effort mode: each event commits independently, errors are collected
        for (index, batch_event) in req.events.iter().enumerate() {
            let result = process_batch_event(
                &state,
                auth.user_id,
                session_id,
                batch_event,
                &mut active_container_id,
                &mut active_item_id,
                index,
            )
            .await;

            match result {
                Ok(batch_result) => {
                    // CB-6: Count inline per result type.
                    match &batch_result {
                        StockerBatchResult::Created { .. } => {
                            items_scanned += 1;
                            items_created += 1;
                        }
                        StockerBatchResult::Moved { .. } => {
                            items_scanned += 1;
                            items_moved += 1;
                        }
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
        state
            .session_repository
            .update_stats(
                session_id,
                active_container_id,
                active_item_id,
                items_scanned,
                items_created,
                items_moved,
                items_errored,
            )
            .await?;
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
    active_item_id: &mut Option<Uuid>,
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
        active_item_id,
        index,
    )
    .await?;
    state.event_store.commit_and_notify(tx).await?;
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
    active_item_id: &mut Option<Uuid>,
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
        StockerBatchEvent::SetContext { container_id, .. } => {
            let is_container: Option<bool> =
                sqlx::query_scalar("SELECT is_container FROM items WHERE id = $1 AND is_deleted = FALSE")
                    .bind(container_id)
                    .fetch_optional(&mut **tx)
                    .await?
                    .ok_or_else(|| AppError::NotFound(format!("Container {container_id} not found")))?;

            if !is_container.unwrap_or(false) {
                return Err(AppError::BadRequest(format!("Item {container_id} is not a container")));
            }

            *active_container_id = Some(*container_id);

            Ok(StockerBatchResult::ContextSet {
                index,
                status: "ok".into(),
                container_id: *container_id,
            })
        }
        StockerBatchEvent::MoveItem {
            item_id, coordinate, ..
        } => {
            let container_id = active_container_id
                .ok_or_else(|| AppError::BadRequest("No active container set. Send set_context first.".into()))?;

            // Validate the item exists and is not deleted.
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE id = $1 AND is_deleted = FALSE)")
                    .bind(item_id)
                    .fetch_one(&mut **tx)
                    .await?;

            if !exists {
                return Err(AppError::NotFound(format!("Item {item_id} not found")));
            }

            let move_req = MoveItemRequest {
                container_id,
                coordinate: coordinate.clone(),
            };

            let stored = state
                .item_commands
                .move_item_in_tx(tx, *item_id, &move_req, actor_id, &metadata)
                .await?;

            // Track this item as the active item for camera attachment
            *active_item_id = Some(*item_id);

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
            let container_id = active_container_id
                .ok_or_else(|| AppError::BadRequest("No active container set. Send set_context first.".into()))?;

            let item_id = Uuid::new_v4();

            // Barcodes are optional. A non-empty scanned barcode string is used directly;
            // empty means the item has no barcode yet (can be assigned later via
            // POST /items/{id}/barcode).
            let system_barcode: Option<String> = if barcode.is_empty() {
                None
            } else if barcode.chars().count() > 32 {
                // M-29: Validate barcode length (same as direct assignment endpoint)
                return Err(AppError::BadRequest(format!(
                    "Barcode exceeds 32 characters: '{}'",
                    barcode
                )));
            } else {
                Some(barcode.clone())
            };

            // When creating a container with a type, inherit the type's default schema.
            // SR-1: Propagate errors so an invalid container_type_id fails the event
            // rather than silently creating a container with no schema.
            let location_schema: Option<serde_json::Value> = match (is_container, container_type_id) {
                (Some(true), Some(type_id)) => {
                    let ct = state.container_type_queries.get_by_id(*type_id).await?;
                    ct.default_location_schema
                }
                _ => None,
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
                location_schema,
                max_capacity_cc: None,
                max_weight_grams: None,
                dimensions: None,
                weight_grams: None,
                is_fungible: *is_fungible,
                fungible_quantity: *fungible_quantity,
                fungible_unit: fungible_unit.clone(),
                external_codes: external_codes.clone(),
                condition: condition.clone(),
                currency: None,
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

            let needs_details = name.as_deref().is_none_or(|n| n.is_empty());

            // Track this newly created item as the active item for camera attachment
            *active_item_id = Some(item_id);

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
    // Revoke all camera tokens when session ends — log but don't block on failure.
    if let Err(e) = state
        .session_repository
        .revoke_all_camera_tokens(id, auth.user_id)
        .await
    {
        tracing::warn!("Failed to revoke camera tokens for session {id}: {e}");
    }
    let session = state.session_repository.end_session(id, auth.user_id).await?;
    Ok(Json(session))
}

#[derive(Debug, Deserialize)]
struct StreamQuery {
    token: String,
}

/// SSE stream of session events. Wakes up on any `event_store` commit via the
/// process-wide broadcast channel and pushes session-scoped events to the
/// client. A 30-second safety interval acts as a heartbeat and handles
/// broadcast lag (`RecvError::Lagged`) by re-querying from the last seen id.
/// Uses a query-param token since `EventSource` can't set headers.
async fn session_event_stream(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Query(q): Query<StreamQuery>,
) -> AppResult<Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>>> {
    // Authenticate via query param token (EventSource can't set headers)
    let claims = crate::auth::jwt::decode_access_token(&q.token, &state.config.jwt_secret)?;
    let user_id = claims.sub;

    // Verify session exists and belongs to user
    let _session = state
        .session_repository
        .get_active_for_user(session_id, user_id)
        .await?;

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, std::convert::Infallible>>(16);

    let pool = state.pool.clone();
    let session_repo = state.session_repository.clone();
    let mut notify = state.event_store.subscribe();
    let sid = session_id.to_string();

    tokio::spawn(async move {
        let mut last_event_id: i64 = 0;
        let mut safety = tokio::time::interval(std::time::Duration::from_secs(30));
        // Fire the initial tick immediately so the client gets a snapshot on
        // connect; subsequent safety ticks are spaced at the configured rate.
        safety.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // First pass always sends a snapshot so the client gets initial state.
        let mut force_send = true;

        loop {
            // `woken_by_notify` is true when a broadcast wake-up raced ahead of
            // the safety tick. When that wake brings no session-relevant events
            // (a commit for an unrelated session), we skip the SSE send to
            // avoid pointless client re-renders.
            let woken_by_notify;
            tokio::select! {
                recv = notify.recv() => {
                    // RecvError::Lagged just means we missed some wake-ups; we
                    // re-query from last_event_id anyway so nothing is lost.
                    // RecvError::Closed means the sender was dropped (app
                    // shutdown) — exit the loop cleanly.
                    if matches!(recv, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                        return;
                    }
                    woken_by_notify = true;
                }
                _ = safety.tick() => {
                    woken_by_notify = false;
                }
            }

            // Check session state; emit session_ended and exit on close.
            let session = match session_repo.get_session_by_id(session_id).await {
                Ok(s) => s,
                Err(_) => continue,
            };
            if session.ended_at.is_some() {
                let _ = tx.send(Ok(Event::default().event("session_ended").data("{}"))).await;
                return;
            }

            let session_json = match serde_json::to_string(&session) {
                Ok(j) => j,
                Err(_) => continue,
            };

            // Fetch events tagged with this session_id…
            let events: Vec<crate::models::event::StoredEvent> = sqlx::query_as(
                r#"
                SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
                FROM event_store
                WHERE metadata->>'session_id' = $1 AND id > $2
                ORDER BY id ASC
                LIMIT 50
                "#,
            )
            .bind(&sid)
            .bind(last_event_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            // …plus events on the active item (camera uploads lack session_id).
            let mut item_events: Vec<crate::models::event::StoredEvent> = Vec::new();
            if let Some(item_id) = session.active_item_id {
                item_events = sqlx::query_as(
                    r#"
                    SELECT id, event_id, aggregate_id, aggregate_type, event_type, event_data, metadata, actor_id, created_at, sequence_number, schema_version
                    FROM event_store
                    WHERE aggregate_id = $1 AND id > $2
                    ORDER BY id ASC
                    LIMIT 50
                    "#,
                )
                .bind(item_id)
                .bind(last_event_id)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();
            }

            // Merge + dedupe by id.
            let mut all_events = events;
            let seen: std::collections::HashSet<i64> = all_events.iter().map(|e| e.id).collect();
            for e in item_events {
                if !seen.contains(&e.id) {
                    all_events.push(e);
                }
            }
            all_events.sort_by_key(|e| e.id);

            if let Some(newest) = all_events.last() {
                last_event_id = newest.id;
            }

            // Skip the send when a broadcast wake produced nothing for this
            // session (commit was for another session). Always send on the
            // first pass (initial snapshot) and on safety ticks (heartbeat +
            // session state refresh).
            if woken_by_notify && !force_send && all_events.is_empty() {
                continue;
            }
            force_send = false;

            let payload = serde_json::json!({
                "session": serde_json::from_str::<serde_json::Value>(&session_json).unwrap_or_default(),
                "events": all_events,
            });

            // Client gone → exit cleanly.
            if tx.send(Ok(Event::default().event("update").data(payload.to_string()))).await.is_err() {
                return;
            }
        }
    });

    Ok(Sse::new(tokio_stream::wrappers::ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

// ── Camera link management (JWT-authenticated) ──────────────────────────

const MAX_CAMERA_DEVICE_NAME_LEN: usize = 128;
const DEFAULT_CAMERA_TOKEN_HOURS: u32 = 24;
const MAX_CAMERA_TOKEN_HOURS: u32 = 168; // 7 days

/// Create a camera link token for a session.
async fn create_camera_link(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
    body: Option<Json<CreateCameraLinkRequest>>,
) -> AppResult<(StatusCode, Json<CameraLinkResponse>)> {
    auth.require_role("member")?;
    // Verify session exists, belongs to user, and is active
    let _session = state
        .session_repository
        .get_active_for_user(session_id, auth.user_id)
        .await?;

    let req = body.map(|b| b.0).unwrap_or(CreateCameraLinkRequest {
        device_name: None,
        expires_in_hours: None,
    });

    if let Some(ref name) = req.device_name {
        if name.chars().count() > MAX_CAMERA_DEVICE_NAME_LEN {
            return Err(AppError::BadRequest(format!(
                "device_name exceeds maximum length of {MAX_CAMERA_DEVICE_NAME_LEN} characters"
            )));
        }
        if name.chars().any(|c| c.is_control()) {
            return Err(AppError::BadRequest(
                "device_name contains invalid control characters".into(),
            ));
        }
    }

    let hours = req
        .expires_in_hours
        .unwrap_or(DEFAULT_CAMERA_TOKEN_HOURS)
        .clamp(1, MAX_CAMERA_TOKEN_HOURS);
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(i64::from(hours));

    // Generate a cryptographically random token.
    let token_bytes: [u8; 32] = rand::random();
    let token = hex::encode(token_bytes);

    let ct = state
        .session_repository
        .create_camera_token(
            session_id,
            auth.user_id,
            &token,
            req.device_name.as_deref(),
            expires_at,
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CameraLinkResponse {
            token,
            session_id: ct.session_id,
            expires_at: ct.expires_at,
            device_name: ct.device_name,
        }),
    ))
}

/// List active camera links for a session.
async fn list_camera_links(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<Vec<CameraToken>>> {
    // Verify session belongs to user
    let _session = state.session_repository.get_for_user(session_id, auth.user_id).await?;
    let tokens = state.session_repository.list_camera_tokens(session_id).await?;
    Ok(Json(tokens))
}

/// Revoke a specific camera link token.
async fn revoke_camera_link(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((session_id, token_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    auth.require_role("member")?;
    // Verify session belongs to user
    let _session = state.session_repository.get_for_user(session_id, auth.user_id).await?;
    state
        .session_repository
        .revoke_camera_token(token_id, auth.user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Camera device endpoints (token-authenticated) ───────────────────────

/// Validate camera token format (64 hex characters).
fn validate_camera_token_format(token: &str) -> AppResult<()> {
    if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

/// Get session status for a camera device.
async fn camera_status(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> AppResult<Json<CameraSessionStatus>> {
    validate_camera_token_format(&token)?;
    let ct = state.session_repository.get_camera_token(&token).await?;
    let session = state.session_repository.get_session_by_id(ct.session_id).await?;

    Ok(Json(CameraSessionStatus {
        session_id: session.id,
        active_container_id: session.active_container_id,
        active_item_id: session.active_item_id,
        session_ended: session.ended_at.is_some(),
    }))
}

// Allowed MIME types by magic bytes for camera uploads (same as item_routes)
fn camera_mime_to_extension(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

/// Upload an image from a remote camera device. Attaches to the session's active item.
async fn camera_upload(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<CameraUploadResponse>)> {
    validate_camera_token_format(&token)?;
    let ct = state.session_repository.get_camera_token(&token).await?;
    let session = state.session_repository.get_session_by_id(ct.session_id).await?;

    if session.ended_at.is_some() {
        return Err(AppError::BadRequest("Session has ended".into()));
    }

    // Determine target: active_item_id first, then fall back to active_container_id
    let target_item_id = session
        .active_item_id
        .or(session.active_container_id)
        .ok_or_else(|| AppError::BadRequest("No active item or container in session. Scan an item first.".into()))?;

    let mut file_data: Option<(String, Vec<u8>)> = None;
    let mut caption: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {e}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read file: {e}")))?;

                if data.len() > state.config.max_upload_bytes {
                    return Err(AppError::BadRequest(format!(
                        "File size {} exceeds maximum {} bytes",
                        data.len(),
                        state.config.max_upload_bytes
                    )));
                }

                // SEC-4/SEC-5: Detect MIME type from magic bytes
                let detected_mime = infer::get(&data)
                    .map(|t| t.mime_type())
                    .unwrap_or("application/octet-stream");

                let ext = camera_mime_to_extension(detected_mime).ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "Unsupported file type detected from content ('{detected_mime}'). \
                         Allowed: {}",
                        state.config.allowed_image_mimes.join(", ")
                    ))
                })?;

                if !state.config.allowed_image_mimes.iter().any(|m| m == detected_mime) {
                    return Err(AppError::BadRequest(format!(
                        "File content type '{detected_mime}' is not allowed. \
                         Allowed: {}",
                        state.config.allowed_image_mimes.join(", ")
                    )));
                }

                let file_id = uuid::Uuid::new_v4();
                let safe_filename = format!("{file_id}.{ext}");
                file_data = Some((safe_filename, data.to_vec()));
            }
            "caption" => {
                caption = field.text().await.ok();
            }
            _ => {}
        }
    }

    let (filename, data) = file_data.ok_or_else(|| AppError::BadRequest("No file provided".into()))?;

    let key = state.storage.upload(target_item_id, &filename, &data).await?;
    let url = state.storage.get_url(&key);

    let metadata = EventMetadata::default();
    // CONC-2: If appending the domain event fails, roll back the uploaded file
    let event = match state
        .item_commands
        .add_image(target_item_id, url.clone(), caption, 0, ct.user_id, &metadata)
        .await
    {
        Ok(ev) => ev,
        Err(e) => {
            if let Err(del_err) = state.storage.delete(&key).await {
                tracing::warn!(
                    key = %key,
                    error = %del_err,
                    "Failed to clean up orphaned image after event-store error (camera upload)"
                );
            }
            return Err(e);
        }
    };

    // Count images on the item to return in response.
    // jsonb_array_length returns INT4; cast to INT8 so sqlx binds to i64.
    let image_count: i64 =
        sqlx::query_scalar("SELECT jsonb_array_length(COALESCE(images, '[]'::jsonb))::bigint FROM items WHERE id = $1")
            .bind(target_item_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);

    tracing::info!(
        session_id = %ct.session_id,
        item_id = %target_item_id,
        event_id = %event.event_id,
        "Camera image uploaded"
    );

    // Return 200 (not 201) for camera uploads — the mobile app expects 200.
    Ok((
        StatusCode::OK,
        Json(CameraUploadResponse {
            item_id: target_item_id,
            image_url: url,
            image_count: image_count as usize,
        }),
    ))
}
