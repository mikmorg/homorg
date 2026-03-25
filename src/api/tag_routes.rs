use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::{get, put},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::errors::{AppError, AppResult};
use crate::models::taxonomy::{CreateTagRequest, RenameTagRequest, Tag};
use crate::AppState;

/// DB column tags.name is VARCHAR(100).
const MAX_TAG_NAME_LEN: usize = 100;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_tags).post(create_tag))
        .route("/{id}", get(get_tag).delete(delete_tag))
        .route("/{id}/rename", put(rename_tag))
}

/// List all tags with their item counts.
async fn list_tags(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<Json<Vec<Tag>>> {
    let tags = state.taxonomy_queries.list_tags().await?;
    Ok(Json(tags))
}

/// Get a single tag by ID.
async fn get_tag(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Tag>> {
    let tag = state.taxonomy_queries.get_tag_by_id(id).await?;
    Ok(Json(tag))
}

/// Create a new tag.
async fn create_tag(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(mut req): Json<CreateTagRequest>,
) -> AppResult<(StatusCode, Json<Tag>)> {
    auth.require_role("member")?;
    req.name = req.name.trim().to_string();
    validate_tag_name(&req.name)?;
    let tag = state.taxonomy_queries.create_tag(&req).await?;
    Ok((StatusCode::CREATED, Json(tag)))
}

/// Rename a tag (affects all items using it).
async fn rename_tag(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(mut req): Json<RenameTagRequest>,
) -> AppResult<Json<Tag>> {
    auth.require_role("member")?;
    req.name = req.name.trim().to_string();
    validate_tag_name(&req.name)?;
    let tag = state.taxonomy_queries.rename_tag(id, &req).await?;
    Ok(Json(tag))
}

/// Validate tag name: non-empty and within DB column width.
fn validate_tag_name(name: &str) -> Result<(), AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("Tag name cannot be empty".into()));
    }
    if trimmed.chars().count() > MAX_TAG_NAME_LEN {
        return Err(AppError::BadRequest(format!(
            "Tag name exceeds {MAX_TAG_NAME_LEN} characters"
        )));
    }
    Ok(())
}

/// Delete a tag and remove it from all items.
async fn delete_tag(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    auth.require_role("member")?;
    state.taxonomy_queries.delete_tag(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
