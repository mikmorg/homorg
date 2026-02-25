use axum::{
    extract::{Json, Multipart, Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::errors::{AppError, AppResult};
use crate::models::event::{EventMetadata, StoredEvent};
use crate::models::item::*;
use crate::AppState;

// ── Input length limits ─────────────────────────────────────────────────
const MAX_NAME_LEN: usize = 500;
const MAX_DESCRIPTION_LEN: usize = 10_000;
const MAX_CATEGORY_LEN: usize = 200;
const MAX_TAG_COUNT: usize = 50;
const MAX_TAG_LEN: usize = 100;
const MAX_METADATA_BYTES: usize = 102_400; // 100 KiB
const MAX_EXTERNAL_CODES: usize = 50;
const MAX_CODE_VALUE_LEN: usize = 200;

/// Validate lengths on create requests.
fn validate_create_request(req: &CreateItemRequest) -> Result<(), AppError> {
    if let Some(ref n) = req.name {
        if n.len() > MAX_NAME_LEN {
            return Err(AppError::BadRequest(format!("name exceeds {MAX_NAME_LEN} chars")));
        }
    }
    if let Some(ref d) = req.description {
        if d.len() > MAX_DESCRIPTION_LEN {
            return Err(AppError::BadRequest(format!("description exceeds {MAX_DESCRIPTION_LEN} chars")));
        }
    }
    if let Some(ref c) = req.category {
        if c.len() > MAX_CATEGORY_LEN {
            return Err(AppError::BadRequest(format!("category exceeds {MAX_CATEGORY_LEN} chars")));
        }
    }
    if let Some(ref tags) = req.tags {
        if tags.len() > MAX_TAG_COUNT {
            return Err(AppError::BadRequest(format!("tags count exceeds {MAX_TAG_COUNT}")));
        }
        for t in tags {
            if t.len() > MAX_TAG_LEN {
                return Err(AppError::BadRequest(format!("tag exceeds {MAX_TAG_LEN} chars")));
            }
        }
    }
    if let Some(ref m) = req.metadata {
        if m.to_string().len() > MAX_METADATA_BYTES {
            return Err(AppError::BadRequest(format!("metadata exceeds {MAX_METADATA_BYTES} bytes")));
        }
    }
    if let Some(ref codes) = req.external_codes {
        if codes.len() > MAX_EXTERNAL_CODES {
            return Err(AppError::BadRequest(format!("external_codes count exceeds {MAX_EXTERNAL_CODES}")));
        }
        for c in codes {
            if c.value.len() > MAX_CODE_VALUE_LEN {
                return Err(AppError::BadRequest(format!("external code value exceeds {MAX_CODE_VALUE_LEN} chars")));
            }
        }
    }
    Ok(())
}

/// Validate lengths on update requests.
fn validate_update_request(req: &UpdateItemRequest) -> Result<(), AppError> {
    if let Some(ref n) = req.name {
        if n.len() > MAX_NAME_LEN {
            return Err(AppError::BadRequest(format!("name exceeds {MAX_NAME_LEN} chars")));
        }
    }
    if let Some(ref d) = req.description {
        if d.len() > MAX_DESCRIPTION_LEN {
            return Err(AppError::BadRequest(format!("description exceeds {MAX_DESCRIPTION_LEN} chars")));
        }
    }
    if let Some(ref c) = req.category {
        if c.len() > MAX_CATEGORY_LEN {
            return Err(AppError::BadRequest(format!("category exceeds {MAX_CATEGORY_LEN} chars")));
        }
    }
    if let Some(ref tags) = req.tags {
        if tags.len() > MAX_TAG_COUNT {
            return Err(AppError::BadRequest(format!("tags count exceeds {MAX_TAG_COUNT}")));
        }
        for t in tags {
            if t.len() > MAX_TAG_LEN {
                return Err(AppError::BadRequest(format!("tag exceeds {MAX_TAG_LEN} chars")));
            }
        }
    }
    if let Some(ref m) = req.metadata {
        if m.to_string().len() > MAX_METADATA_BYTES {
            return Err(AppError::BadRequest(format!("metadata exceeds {MAX_METADATA_BYTES} bytes")));
        }
    }
    Ok(())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create_item))
        .route("/{id}", get(get_item).put(update_item).delete(delete_item))
        .route("/{id}/restore", post(restore_item))
        .route("/{id}/move", post(move_item))
        .route("/{id}/history", get(get_history))
        .route("/{id}/images", post(upload_image))
        .route("/{id}/images/{idx}", delete(remove_image))
        .route("/{id}/external-codes", post(add_external_code))
        .route(
            "/{id}/external-codes/{code_type}/{value}",
            delete(remove_external_code),
        )
        .route("/{id}/quantity", post(adjust_quantity))
}

/// Create a new item.
async fn create_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(mut req): Json<CreateItemRequest>,
) -> AppResult<(StatusCode, Json<StoredEvent>)> {
    auth.require_role("member")?;
    validate_create_request(&req)?;

    // Auto-generate barcode if not provided
    if req.system_barcode.is_none() {
        let generated = state.barcode_commands.generate_barcode().await?;
        req.system_barcode = Some(generated.barcode);
    }

    let item_id = Uuid::new_v4();
    let metadata = EventMetadata::default();

    let event = state
        .item_commands
        .create_item(item_id, &req, auth.user_id, &metadata)
        .await?;

    Ok((StatusCode::CREATED, Json(event)))
}

/// Get full item detail with ancestor breadcrumbs.
async fn get_item(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ItemDetail>> {
    let detail = state.item_queries.get_by_id(id).await?;
    Ok(Json(detail))
}

/// Partial update of item metadata fields.
async fn update_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateItemRequest>,
) -> AppResult<Json<StoredEvent>> {
    auth.require_role("member")?;
    validate_update_request(&req)?;
    let metadata = EventMetadata::default();
    let event = state
        .item_commands
        .update_item(id, &req, auth.user_id, &metadata)
        .await?;
    Ok(Json(event))
}

/// Soft-delete an item.
async fn delete_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    auth.require_role("member")?;
    let metadata = EventMetadata::default();
    state
        .item_commands
        .delete_item(id, None, auth.user_id, &metadata)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Restore a soft-deleted item.
async fn restore_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StoredEvent>> {
    auth.require_role("member")?;
    let metadata = EventMetadata::default();
    let event = state
        .item_commands
        .restore_item(id, auth.user_id, &metadata)
        .await?;
    Ok(Json(event))
}

/// Move item to a different container.
async fn move_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<MoveItemRequest>,
) -> AppResult<Json<StoredEvent>> {
    auth.require_role("member")?;
    let metadata = EventMetadata::default();
    let event = state
        .item_commands
        .move_item(id, &req, auth.user_id, &metadata)
        .await?;
    Ok(Json(event))
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    after_seq: Option<i64>,
    limit: Option<i64>,
}

/// Get paginated event history for an item.
async fn get_history(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<Vec<StoredEvent>>> {
    let limit = q.limit.unwrap_or(50).min(200);
    let events = state.item_queries.get_history(id, q.after_seq, limit).await?;
    Ok(Json(events))
}

/// Upload an image via multipart form data.
async fn upload_image(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<StoredEvent>)> {
    auth.require_role("member")?;

    let mut file_data: Option<(String, Vec<u8>)> = None;
    let mut caption: Option<String> = None;
    let mut order: i32 = 0;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {e}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                // Validate MIME type
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                if !state.config.allowed_image_mimes.iter().any(|m| m == &content_type) {
                    return Err(AppError::BadRequest(format!(
                        "Unsupported file type '{content_type}'. Allowed: {}",
                        state.config.allowed_image_mimes.join(", ")
                    )));
                }

                let filename = field
                    .file_name()
                    .unwrap_or("upload.bin")
                    .to_string();
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

                file_data = Some((filename, data.to_vec()));
            }
            "caption" => {
                caption = field.text().await.ok();
            }
            "order" => {
                if let Ok(text) = field.text().await {
                    order = text.parse().unwrap_or(0);
                }
            }
            _ => {}
        }
    }

    let (filename, data) = file_data.ok_or_else(|| AppError::BadRequest("No file provided".into()))?;

    let key = state.storage.upload(id, &filename, &data).await?;
    let url = state.storage.get_url(&key);

    let metadata = EventMetadata::default();
    let event = state
        .item_commands
        .add_image(id, url, caption, order, auth.user_id, &metadata)
        .await?;

    Ok((StatusCode::CREATED, Json(event)))
}

/// Remove an image by its index in the images array.
async fn remove_image(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((id, idx)): Path<(Uuid, usize)>,
) -> AppResult<Json<StoredEvent>> {
    auth.require_role("member")?;

    let metadata = EventMetadata::default();
    // TOCTOU-safe: index resolved inside a transaction
    let (event, path) = state
        .item_commands
        .remove_image_by_index(id, idx, auth.user_id, &metadata)
        .await?;

    // Clean up file from storage (best-effort, log on failure)
    if let Err(e) = state.storage.delete(&path).await {
        tracing::warn!(path = %path, error = %e, "Failed to delete image file from storage");
    }

    Ok(Json(event))
}

/// Add an external code (UPC, EAN, ISBN, etc.)
async fn add_external_code(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AddExternalCodeRequest>,
) -> AppResult<(StatusCode, Json<StoredEvent>)> {
    auth.require_role("member")?;
    let metadata = EventMetadata::default();
    let event = state
        .item_commands
        .add_external_code(id, req.code_type, req.value, auth.user_id, &metadata)
        .await?;
    Ok((StatusCode::CREATED, Json(event)))
}

/// Remove an external code by type and value.
async fn remove_external_code(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((id, code_type, value)): Path<(Uuid, String, String)>,
) -> AppResult<Json<StoredEvent>> {
    auth.require_role("member")?;
    let metadata = EventMetadata::default();
    let event = state
        .item_commands
        .remove_external_code(id, code_type, value, auth.user_id, &metadata)
        .await?;
    Ok(Json(event))
}

/// Adjust fungible quantity.
async fn adjust_quantity(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AdjustQuantityRequest>,
) -> AppResult<Json<StoredEvent>> {
    auth.require_role("member")?;
    let metadata = EventMetadata::default();
    let event = state
        .item_commands
        .adjust_quantity(id, &req, auth.user_id, &metadata)
        .await?;
    Ok(Json(event))
}
