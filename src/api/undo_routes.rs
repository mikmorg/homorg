use axum::{
    extract::{Json, Path, State},
    routing::post,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::errors::AppResult;
use crate::models::event::StoredEvent;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/event/{event_id}", post(undo_event))
        .route("/batch", post(undo_batch))
}

/// Undo a single event by generating a compensating event.
async fn undo_event(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(event_id): Path<Uuid>,
) -> AppResult<Json<StoredEvent>> {
    auth.require_role("member")?;
    let event = state
        .undo_commands
        .undo_event(event_id, auth.user_id)
        .await?;
    Ok(Json(event))
}

#[derive(Debug, Deserialize)]
struct UndoBatchBody {
    event_ids: Option<Vec<Uuid>>,
    session_id: Option<String>,
}

/// Undo a batch of events or an entire session.
async fn undo_batch(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<UndoBatchBody>,
) -> AppResult<Json<Vec<StoredEvent>>> {
    auth.require_role("member")?;

    // Validate batch size limit
    if let Some(ref ids) = body.event_ids {
        if ids.len() > state.config.max_batch_size {
            return Err(crate::errors::AppError::BadRequest(format!(
                "Batch size {} exceeds maximum {}",
                ids.len(),
                state.config.max_batch_size
            )));
        }
    }

    // API-3: If both are provided, it's ambiguous — reject with a clear error.
    match (&body.session_id, &body.event_ids) {
        (Some(_), Some(_)) => {
            return Err(crate::errors::AppError::BadRequest(
                "Provide either event_ids or session_id, not both".into(),
            ));
        }
        (None, None) => {
            return Err(crate::errors::AppError::BadRequest(
                "Provide either event_ids or session_id".into(),
            ));
        }
        _ => {}
    }

    let events = if let Some(session_id) = &body.session_id {
        state
            .undo_commands
            .undo_session(session_id, auth.user_id, state.config.max_batch_size)
            .await?
    } else {
        // SAFETY: validated above that event_ids is Some when session_id is None.
        state
            .undo_commands
            .undo_batch(body.event_ids.as_ref().unwrap(), auth.user_id)
            .await?
    };

    Ok(Json(events))
}
