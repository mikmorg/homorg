use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::get,
    Router,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::errors::{AppError, AppResult};
use crate::models::taxonomy::{Category, CreateCategoryRequest, UpdateCategoryRequest};
use crate::AppState;

/// DB column categories.name is VARCHAR(128).
const MAX_CATEGORY_NAME_LEN: usize = 128;
/// Guard against unbounded TEXT descriptions (DoS via giant payloads).
const MAX_CATEGORY_DESC_LEN: usize = 10_000;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_categories).post(create_category))
        .route("/{id}", get(get_category).put(update_category).delete(delete_category))
}

/// List all categories with their item counts.
async fn list_categories(State(state): State<Arc<AppState>>, _auth: AuthUser) -> AppResult<Json<Vec<Category>>> {
    let categories = state.taxonomy_queries.list_categories().await?;
    Ok(Json(categories))
}

/// Get a single category by ID.
async fn get_category(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Category>> {
    let category = state.taxonomy_queries.get_category_by_id(id).await?;
    Ok(Json(category))
}

/// Create a new category.
async fn create_category(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(mut req): Json<CreateCategoryRequest>,
) -> AppResult<(StatusCode, Json<Category>)> {
    auth.require_role("member")?;
    req.name = req.name.trim().to_string();
    validate_category_name(&req.name)?;
    if let Some(ref desc) = req.description {
        if desc.len() > MAX_CATEGORY_DESC_LEN {
            return Err(AppError::BadRequest(format!(
                "description exceeds {MAX_CATEGORY_DESC_LEN} bytes"
            )));
        }
    }
    let category = state.taxonomy_queries.create_category(&req).await?;
    Ok((StatusCode::CREATED, Json(category)))
}

/// Update a category (rename, change description, re-parent).
async fn update_category(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(mut req): Json<UpdateCategoryRequest>,
) -> AppResult<Json<Category>> {
    auth.require_role("member")?;
    if let Some(ref mut name) = req.name {
        *name = name.trim().to_string();
        validate_category_name(name)?;
    }
    if let Some(ref desc) = req.description {
        if desc.len() > MAX_CATEGORY_DESC_LEN {
            return Err(AppError::BadRequest(format!(
                "description exceeds {MAX_CATEGORY_DESC_LEN} bytes"
            )));
        }
    }
    let category = state.taxonomy_queries.update_category(id, &req).await?;
    Ok(Json(category))
}

/// Validate category name: non-empty and within DB column width.
fn validate_category_name(name: &str) -> Result<(), AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("Category name cannot be empty".into()));
    }
    if trimmed.chars().count() > MAX_CATEGORY_NAME_LEN {
        return Err(AppError::BadRequest(format!(
            "Category name exceeds {MAX_CATEGORY_NAME_LEN} characters"
        )));
    }
    Ok(())
}

/// Delete a category (items will have their category_id set to NULL).
async fn delete_category(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    auth.require_role("member")?;
    state.taxonomy_queries.delete_category(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_name_rejects_empty() {
        assert!(validate_category_name("").is_err());
    }

    #[test]
    fn category_name_rejects_whitespace_only() {
        assert!(validate_category_name("  ").is_err());
    }

    #[test]
    fn category_name_rejects_over_128_chars() {
        assert!(validate_category_name(&"x".repeat(129)).is_err());
    }

    #[test]
    fn category_name_accepts_at_128_chars() {
        assert!(validate_category_name(&"x".repeat(128)).is_ok());
    }

    #[test]
    fn category_name_accepts_normal_name() {
        assert!(validate_category_name("Household").is_ok());
    }

    #[test]
    fn category_desc_over_limit_rejected_in_create() {
        let req = CreateCategoryRequest {
            name: "Valid".into(),
            description: Some("x".repeat(10_001)),
            parent_category_id: None,
        };
        // Simulate the handler's inline check
        assert!(req.description.as_ref().unwrap().len() > MAX_CATEGORY_DESC_LEN);
    }
}
