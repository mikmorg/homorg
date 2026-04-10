use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::auth::middleware::AuthUser;
use crate::errors::AppResult;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(export_all))
}

#[derive(Debug, Serialize)]
struct ExportData {
    exported_at: chrono::DateTime<chrono::Utc>,
    version: &'static str,
    items: Vec<serde_json::Value>,
    events: Vec<serde_json::Value>,
    users: Vec<serde_json::Value>,
}

/// Export all data as JSON (admin only).
/// Returns items, event history, and users for data portability.
async fn export_all(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<Json<ExportData>> {
    auth.require_role("admin")?;

    // Fetch all items (including deleted for completeness)
    let items: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT row_to_json(i) FROM items i ORDER BY created_at ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    // Fetch all events
    let events: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT row_to_json(e) FROM event_store e ORDER BY id ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    // Fetch all users (excluding password hashes)
    let users: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT json_build_object(\
            'id', id, 'username', username, 'display_name', display_name, \
            'role', role, 'is_active', is_active, 'created_at', created_at, \
            'container_id', container_id\
        ) FROM users ORDER BY created_at ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(ExportData {
        exported_at: chrono::Utc::now(),
        version: env!("CARGO_PKG_VERSION"),
        items,
        events,
        users,
    }))
}
