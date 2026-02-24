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

    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at")
        .fetch_all(&state.pool)
        .await?;

    let public: Vec<UserPublic> = users.into_iter().map(Into::into).collect();
    Ok(Json(public))
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

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {id} not found")))?;

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
        sqlx::query("UPDATE users SET display_name = $1, updated_at = NOW() WHERE id = $2")
            .bind(display_name)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

    // Update password if provided
    if let Some(ref password) = req.password {
        if password.len() < 8 {
            return Err(AppError::BadRequest(
                "Password must be at least 8 characters".into(),
            ));
        }
        let pw_hash = hash_password(password)?;
        sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
            .bind(&pw_hash)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;

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

    sqlx::query("UPDATE users SET role = $1, updated_at = NOW() WHERE id = $2")
        .bind(&req.role)
        .bind(id)
        .execute(&state.pool)
        .await?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;

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

    sqlx::query("UPDATE users SET is_active = FALSE, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    // Revoke all refresh tokens for the deactivated user
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
