use axum::{
    extract::{Path, Query, State},
    routing::{get, put},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::errors::{AppError, AppResult};
use crate::models::event::{EventMetadata, StoredEvent};
use crate::models::item::{AncestorEntry, ContainerStats, ItemSummary};
use crate::AppState;

/// Maximum serialized size of a container location schema (64 KiB).
const MAX_SCHEMA_BYTES: usize = 65_536;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/{id}/children", get(get_children))
        .route("/{id}/descendants", get(get_descendants))
        .route("/{id}/ancestors", get(get_ancestors))
        .route("/{id}/stats", get(get_stats))
        .route("/{id}/schema", put(update_schema))
}

#[derive(Debug, Deserialize)]
struct ChildrenQuery {
    cursor: Option<Uuid>,
    limit: Option<i64>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// Get direct children of a container, paginated and sortable.
async fn get_children(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<ChildrenQuery>,
) -> AppResult<Json<Vec<ItemSummary>>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let items = state
        .container_queries
        .get_children(
            id,
            q.cursor,
            limit,
            q.sort_by.as_deref(),
            q.sort_dir.as_deref(),
        )
        .await?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
struct DescendantsQuery {
    max_depth: Option<i32>,
    limit: Option<i64>,
}

/// Get full subtree via LTREE (with optional max_depth).
async fn get_descendants(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<DescendantsQuery>,
) -> AppResult<Json<Vec<ItemSummary>>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 1000);
    // N2: Clamp max_depth to at least 1; 0 or negative would return no children.
    let max_depth = q.max_depth.map(|d| d.max(1));
    let items = state
        .container_queries
        .get_descendants(id, max_depth, limit)
        .await?;
    Ok(Json(items))
}

/// Get ancestor breadcrumb path to Root.
async fn get_ancestors(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<AncestorEntry>>> {
    let ancestors = state.container_queries.get_ancestors(id).await?;
    Ok(Json(ancestors))
}

/// Get container statistics (child count, weight, volume utilization).
async fn get_stats(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ContainerStats>> {
    let stats = state.container_queries.get_stats(id).await?;
    Ok(Json(stats))
}

#[derive(Debug, Deserialize)]
struct SchemaBody {
    schema: serde_json::Value,
    #[serde(default)]
    label_renames: std::collections::HashMap<String, String>,
}

/// Update a container's location schema.
async fn update_schema(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SchemaBody>,
) -> AppResult<Json<StoredEvent>> {
    auth.require_role("member")?;

    // SEC-3: Reject oversized schema payloads before persisting to the event store.
    let schema_bytes = body.schema.to_string().len();
    if schema_bytes > MAX_SCHEMA_BYTES {
        return Err(AppError::BadRequest(format!(
            "schema exceeds maximum size of {MAX_SCHEMA_BYTES} bytes (got {schema_bytes})"
        )));
    }

    // N1: Validate label_renames size to prevent memory amplification.
    const MAX_RENAMES: usize = 200;
    const MAX_RENAME_KEY_BYTES: usize = 256;
    if body.label_renames.len() > MAX_RENAMES {
        return Err(AppError::BadRequest(format!(
            "label_renames exceeds maximum of {MAX_RENAMES} entries"
        )));
    }
    for (k, v) in &body.label_renames {
        if k.len() > MAX_RENAME_KEY_BYTES || v.len() > MAX_RENAME_KEY_BYTES {
            return Err(AppError::BadRequest(
                "label_renames key or value exceeds 256 bytes".into(),
            ));
        }
    }

    let metadata = EventMetadata::default();
    let event = state
        .item_commands
        .update_container_schema(id, body.schema, body.label_renames, auth.user_id, &metadata)
        .await?;
    Ok(Json(event))
}
