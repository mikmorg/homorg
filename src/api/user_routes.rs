use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::{get, put},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::auth::password::{hash_password, verify_password};
use crate::constants::{MAX_DISPLAY_NAME_LEN, PASSWORD_MAX_LEN, PASSWORD_MIN_LEN};
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
async fn list_users(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<Json<Vec<UserPublic>>> {
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
            return Err(AppError::BadRequest(format!(
                "Password must be {PASSWORD_MIN_LEN}–{PASSWORD_MAX_LEN} characters"
            )));
        }
    }
    // SEC-6: Self-service password change requires the current password so that
    // a stolen access token cannot be used to permanently hijack the account.
    let is_self = auth.user_id == id;
    if req.password.is_some() && is_self && req.current_password.is_none() {
        return Err(AppError::BadRequest(
            "current_password is required to change your password".into(),
        ));
    }
    if let Some(ref dn) = req.display_name {
        if dn.trim().is_empty() {
            return Err(AppError::BadRequest("display_name cannot be blank".into()));
        }
        if dn.chars().count() > MAX_DISPLAY_NAME_LEN {
            return Err(AppError::BadRequest(format!(
                "display_name exceeds {MAX_DISPLAY_NAME_LEN} chars"
            )));
        }
    }

    // Hash password outside the transaction (CPU-intensive work).
    // Pre-verify current_password first so we don't waste CPU hashing when the current
    // password is wrong — verify is fast (~10ms), hash is slow (~100ms).
    if req.password.is_some() && is_self {
        let stored_hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
        if let Some(ref hash) = stored_hash {
            let current_pw = req.current_password.as_deref().unwrap_or("");
            if !verify_password(current_pw, hash).await? {
                return Err(AppError::Unauthorized);
            }
        }
    }
    let pw_hash = match req.password {
        Some(ref password) => Some(hash_password(password).await?),
        None => None,
    };

    // Apply all updates atomically
    let mut tx = state.pool.begin().await?;

    // EH-4: Lock and verify user exists before updating to produce a proper 404.
    // Also fetch password_hash so we can verify current_password for self-service changes.
    let found: Option<(Uuid, String)> = sqlx::query_as("SELECT id, password_hash FROM users WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let (_, stored_hash) = found.ok_or_else(|| AppError::NotFound(format!("User {id} not found")))?;

    // SEC-6: Verify current password in-transaction as a second safeguard (TOCTOU safety).
    if req.password.is_some() && is_self {
        let current_pw = req.current_password.as_deref().unwrap_or("");
        if !verify_password(current_pw, &stored_hash).await? {
            return Err(AppError::Unauthorized);
        }
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

    // SEC-5: Invalidate all existing refresh tokens when the password changes so that
    // a stolen token cannot be exchanged after the legitimate user rotates their password.
    // Run outside the transaction — if this fails the password was still changed (the user
    // retains protection) and stale tokens will expire naturally within jwt_refresh_ttl_days.
    if pw_hash.is_some() {
        state.token_repository.revoke_all_for_user(id).await?;
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

    // SEC-2: Last-admin guard — prevent leaving zero admins.
    // Run inside a transaction to close the TOCTOU window between counting and updating.
    let mut tx = state.pool.begin().await?;

    let current_role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;

    let current_role = current_role.ok_or_else(|| AppError::NotFound(format!("User {id} not found")))?;

    if current_role == "admin" && req.role != "admin" {
        // Count remaining active admins *excluding* the one being demoted
        let remaining_admins: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = TRUE AND id != $1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;

        if remaining_admins == 0 {
            return Err(AppError::BadRequest(
                "Cannot demote the last admin. Promote another user to admin first.".into(),
            ));
        }
    }

    sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
        .bind(&req.role)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

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

    // SEC-7: Run inside a transaction with FOR UPDATE to prevent a race where
    // two concurrent requests deactivate each other, leaving zero admins.
    let mut tx = state.pool.begin().await?;

    let role: Option<String> =
        sqlx::query_scalar("SELECT role FROM users WHERE id = $1 AND is_active = TRUE FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;

    let role = role.ok_or_else(|| AppError::NotFound(format!("User {id} not found")))?;

    if role == "admin" {
        let remaining_admins: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = TRUE AND id != $1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;

        if remaining_admins == 0 {
            return Err(AppError::BadRequest(
                "Cannot deactivate the last admin. Promote another user first.".into(),
            ));
        }
    }

    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Revoke all refresh tokens for the deactivated user
    sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}
