use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::{get, put},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::auth::password::hash_password;
use crate::constants::{PASSWORD_MIN_LEN, PASSWORD_MAX_LEN};
use crate::errors::{AppError, AppResult};
use crate::models::user::*;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_users))
        .route("/{id}", get(get_user).put(update_user).delete(deactivate_user))
        .route("/{id}/role", put(update_role))
}

/// List all household members (admin only).
async fn list_users(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<Vec<UserPublic>>> {
    auth.require_role("admin")?;
    let users = state.user_queries.list_all().await?;
    Ok(Json(users))
}

/// Get user detail (own profile or admin).
async fn get_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserPublic>> {
    // Users can view their own profile; admins can view any
    if auth.user_id != id {
        auth.require_role("admin")?;
    }

    let user = state.user_queries.find_by_id(id).await?;
    Ok(Json(user.into()))
}

/// Update user profile (own or admin).
async fn update_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> AppResult<Json<UserPublic>> {
    if auth.user_id != id {
        auth.require_role("admin")?;
    }

    // Update display_name if provided
    if let Some(ref display_name) = req.display_name {
        state.user_queries.update_display_name(id, display_name).await?;
    }

    // Update password if provided
    if let Some(ref password) = req.password {
        if password.len() < PASSWORD_MIN_LEN || password.len() > PASSWORD_MAX_LEN {
            return Err(AppError::BadRequest(
                format!("Password must be {PASSWORD_MIN_LEN}–{PASSWORD_MAX_LEN} characters"),
            ));
        }
        let pw_hash = hash_password(password)?;
        state.user_queries.update_password(id, &pw_hash).await?;
    }

    let user = state.user_queries.find_by_id(id).await?;
    Ok(Json(user.into()))
}

/// Change a user's role (admin only).
async fn update_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateRoleRequest>,
) -> AppResult<Json<UserPublic>> {
    auth.require_role("admin")?;

    // Validate role value
    if !["admin", "member", "readonly"].contains(&req.role.as_str()) {
        return Err(AppError::BadRequest(format!("Invalid role: {}", req.role)));
    }

    // Prevent demoting self
    if auth.user_id == id && req.role != "admin" {
        return Err(AppError::BadRequest("Cannot demote yourself".into()));
    }

    state.user_queries.update_role(id, &req.role).await?;
    let user = state.user_queries.find_by_id(id).await?;
    Ok(Json(user.into()))
}

/// Deactivate a user (admin only).
async fn deactivate_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    auth.require_role("admin")?;

    if auth.user_id == id {
        return Err(AppError::BadRequest("Cannot deactivate yourself".into()));
    }

    state.user_queries.deactivate(id).await?;

    // Revoke all refresh tokens for the deactivated user
    state.token_repository.revoke_all_for_user(id).await?;

    Ok(StatusCode::NO_CONTENT)
}
