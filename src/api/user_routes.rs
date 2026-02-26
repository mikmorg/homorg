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

    // Validate password up front before starting the transaction
    if let Some(ref password) = req.password {
        let pw_chars = password.chars().count();
        if !(PASSWORD_MIN_LEN..=PASSWORD_MAX_LEN).contains(&pw_chars) {
            return Err(AppError::BadRequest(
                format!("Password must be {PASSWORD_MIN_LEN}–{PASSWORD_MAX_LEN} characters"),
            ));
        }
    }

    // Hash password outside the transaction (CPU-intensive work)
    let pw_hash = match req.password {
        Some(ref password) => Some(hash_password(password).await?),
        None => None,
    };

    // Apply all updates atomically
    let mut tx = state.pool.begin().await?;

    // EH-4: Lock and verify user exists before updating to produce a proper 404.
    let found: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if found.is_none() {
        return Err(AppError::NotFound(format!("User {id} not found")));
    }

    if let Some(ref display_name) = req.display_name {
        sqlx::query("UPDATE users SET display_name = $1 WHERE id = $2")
            .bind(display_name)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    if let Some(ref hash) = pw_hash {
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(hash)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

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
