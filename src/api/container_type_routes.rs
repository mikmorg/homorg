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
use crate::models::container_type::{ContainerType, CreateContainerTypeRequest, UpdateContainerTypeRequest};
use crate::AppState;

// Field limits matching DB schema column widths.
const MAX_CT_NAME_LEN: usize = 128;
const MAX_CT_ICON_LEN: usize = 64;
const MAX_CT_PURPOSE_LEN: usize = 64;
const MAX_CT_DESC_BYTES: usize = 10_000;
const MAX_CT_SCHEMA_BYTES: usize = 65_536;

fn validate_ct_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::BadRequest("Container type name cannot be empty".into()));
    }
    if name.chars().count() > MAX_CT_NAME_LEN {
        return Err(AppError::BadRequest(format!(
            "Container type name exceeds maximum length of {MAX_CT_NAME_LEN} characters"
        )));
    }
    Ok(())
}

fn validate_create_container_type_request(req: &CreateContainerTypeRequest) -> Result<(), AppError> {
    validate_ct_name(&req.name)?;
    if let Some(ref icon) = req.icon {
        if icon.chars().count() > MAX_CT_ICON_LEN {
            return Err(AppError::BadRequest(format!(
                "Icon exceeds maximum length of {MAX_CT_ICON_LEN} characters"
            )));
        }
    }
    if let Some(ref purpose) = req.purpose {
        if !purpose.is_empty() && purpose.chars().count() > MAX_CT_PURPOSE_LEN {
            return Err(AppError::BadRequest(format!(
                "Purpose exceeds maximum length of {MAX_CT_PURPOSE_LEN} characters"
            )));
        }
    }
    if let Some(ref desc) = req.description {
        if desc.len() > MAX_CT_DESC_BYTES {
            return Err(AppError::BadRequest(format!(
                "Description exceeds maximum size of {MAX_CT_DESC_BYTES} bytes"
            )));
        }
    }
    if let Some(ref schema) = req.default_location_schema {
        let serialized = serde_json::to_string(schema).unwrap_or_default();
        if serialized.len() > MAX_CT_SCHEMA_BYTES {
            return Err(AppError::BadRequest(format!(
                "default_location_schema exceeds maximum size of {MAX_CT_SCHEMA_BYTES} bytes"
            )));
        }
    }
    if let Some(ref dims) = req.default_dimensions {
        let serialized = serde_json::to_string(dims).unwrap_or_default();
        if serialized.len() > MAX_CT_SCHEMA_BYTES {
            return Err(AppError::BadRequest(format!(
                "default_dimensions exceeds maximum size of {MAX_CT_SCHEMA_BYTES} bytes"
            )));
        }
    }
    Ok(())
}

fn validate_update_container_type_request(req: &UpdateContainerTypeRequest) -> Result<(), AppError> {
    if let Some(ref name) = req.name {
        validate_ct_name(name)?;
    }
    if let Some(ref icon) = req.icon {
        if icon.chars().count() > MAX_CT_ICON_LEN {
            return Err(AppError::BadRequest(format!(
                "Icon exceeds maximum length of {MAX_CT_ICON_LEN} characters"
            )));
        }
    }
    if let Some(ref purpose) = req.purpose {
        if !purpose.is_empty() && purpose.chars().count() > MAX_CT_PURPOSE_LEN {
            return Err(AppError::BadRequest(format!(
                "Purpose exceeds maximum length of {MAX_CT_PURPOSE_LEN} characters"
            )));
        }
    }
    if let Some(ref desc) = req.description {
        if desc.len() > MAX_CT_DESC_BYTES {
            return Err(AppError::BadRequest(format!(
                "Description exceeds maximum size of {MAX_CT_DESC_BYTES} bytes"
            )));
        }
    }
    if let Some(ref schema) = req.default_location_schema {
        let serialized = serde_json::to_string(schema).unwrap_or_default();
        if serialized.len() > MAX_CT_SCHEMA_BYTES {
            return Err(AppError::BadRequest(format!(
                "default_location_schema exceeds maximum size of {MAX_CT_SCHEMA_BYTES} bytes"
            )));
        }
    }
    if let Some(ref dims) = req.default_dimensions {
        let serialized = serde_json::to_string(dims).unwrap_or_default();
        if serialized.len() > MAX_CT_SCHEMA_BYTES {
            return Err(AppError::BadRequest(format!(
                "default_dimensions exceeds maximum size of {MAX_CT_SCHEMA_BYTES} bytes"
            )));
        }
    }
    Ok(())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_container_types).post(create_container_type))
        .route(
            "/{id}",
            get(get_container_type)
                .put(update_container_type)
                .delete(delete_container_type),
        )
}

/// List all container types.
async fn list_container_types(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<Json<Vec<ContainerType>>> {
    let types = state.container_type_queries.list_all().await?;
    Ok(Json(types))
}

/// Get a single container type by ID.
async fn get_container_type(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ContainerType>> {
    let ct = state.container_type_queries.get_by_id(id).await?;
    Ok(Json(ct))
}

/// Create a new container type.
async fn create_container_type(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<CreateContainerTypeRequest>,
) -> AppResult<(StatusCode, Json<ContainerType>)> {
    auth.require_role("admin")?;
    validate_create_container_type_request(&req)?;
    let ct = state.container_type_queries.create(&req, auth.user_id).await?;
    Ok((StatusCode::CREATED, Json(ct)))
}

/// Update a container type.
async fn update_container_type(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateContainerTypeRequest>,
) -> AppResult<Json<ContainerType>> {
    auth.require_role("admin")?;
    validate_update_container_type_request(&req)?;
    let ct = state.container_type_queries.update(id, &req).await?;
    Ok(Json(ct))
}

/// Delete a container type.
async fn delete_container_type(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    auth.require_role("admin")?;
    state.container_type_queries.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_create() -> CreateContainerTypeRequest {
        CreateContainerTypeRequest {
            name: "Shelf".into(),
            description: None,
            default_max_capacity_cc: None,
            default_max_weight_grams: None,
            default_dimensions: None,
            default_location_schema: None,
            icon: None,
            purpose: None,
        }
    }

    fn minimal_update() -> UpdateContainerTypeRequest {
        UpdateContainerTypeRequest {
            name: None,
            description: None,
            default_max_capacity_cc: None,
            default_max_weight_grams: None,
            default_dimensions: None,
            default_location_schema: None,
            icon: None,
            purpose: None,
        }
    }

    #[test]
    fn ct_name_rejects_empty() {
        assert!(validate_ct_name("").is_err());
    }

    #[test]
    fn ct_name_rejects_over_128_chars() {
        assert!(validate_ct_name(&"x".repeat(129)).is_err());
    }

    #[test]
    fn ct_name_accepts_at_128_chars() {
        assert!(validate_ct_name(&"x".repeat(128)).is_ok());
    }

    #[test]
    fn create_ct_rejects_icon_over_64_chars() {
        let mut req = minimal_create();
        req.icon = Some("x".repeat(65));
        assert!(validate_create_container_type_request(&req).is_err());
    }

    #[test]
    fn create_ct_rejects_purpose_over_64_chars() {
        let mut req = minimal_create();
        req.purpose = Some("x".repeat(65));
        assert!(validate_create_container_type_request(&req).is_err());
    }

    #[test]
    fn create_ct_rejects_description_over_10kb() {
        let mut req = minimal_create();
        req.description = Some("x".repeat(10_001));
        assert!(validate_create_container_type_request(&req).is_err());
    }

    #[test]
    fn create_ct_rejects_schema_over_64kb() {
        let mut req = minimal_create();
        req.default_location_schema = Some(serde_json::json!("x".repeat(65_537)));
        assert!(validate_create_container_type_request(&req).is_err());
    }

    #[test]
    fn create_ct_accepts_valid_request() {
        assert!(validate_create_container_type_request(&minimal_create()).is_ok());
    }

    #[test]
    fn update_ct_rejects_oversized_name() {
        let mut req = minimal_update();
        req.name = Some("x".repeat(129));
        assert!(validate_update_container_type_request(&req).is_err());
    }

    #[test]
    fn update_ct_accepts_partial_update() {
        let mut req = minimal_update();
        req.name = Some("Box".into());
        assert!(validate_update_container_type_request(&req).is_ok());
    }

    #[test]
    fn update_ct_accepts_empty_update() {
        assert!(validate_update_container_type_request(&minimal_update()).is_ok());
    }
}
