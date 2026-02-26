use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::jwt::create_access_token;
use crate::auth::middleware::AuthUser;
use crate::auth::password::{hash_password, verify_password};
use crate::constants::{PASSWORD_MAX_LEN, PASSWORD_MIN_LEN, USERS_ID, is_valid_username};
use crate::errors::{AppError, AppResult};
use crate::models::event::EventMetadata;
use crate::models::item::CreateItemRequest;
use crate::models::user::*;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/setup", post(setup))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/invite", post(create_invite))
        .route("/register", post(register))
}

/// Advisory lock ID for the one-time setup operation.
/// Derived from FNV-1a("homorg:setup") to avoid colliding with other advisory locks.
const ADVISORY_LOCK_SETUP: i64 = 0x1A2B_3C4D_5E6F_0011u64 as i64;

/// Build a [`CreateItemRequest`] for a new user's personal container.
fn build_user_container_request(username: &str, display_name: Option<&str>) -> CreateItemRequest {
    // CB-1: Barcode column is VARCHAR(32). "USR-" prefix = 4 chars → username limited to 28 chars.
    let upper = username.to_uppercase();
    let trimmed = &upper[..upper.len().min(28)];
    let container_barcode = format!("USR-{trimmed}");
    let label = display_name.unwrap_or(username);
    CreateItemRequest {
        system_barcode: Some(container_barcode),
        parent_id: USERS_ID,
        name: Some(format!("{label}'s Items")),
        description: None,
        category: None,
        tags: None,
        is_container: Some(true),
        coordinate: None,
        location_schema: None,
        max_capacity_cc: None,
        max_weight_grams: None,
        dimensions: None,
        weight_grams: None,
        is_fungible: None,
        fungible_quantity: None,
        fungible_unit: None,
        external_codes: None,
        condition: None,
        acquisition_date: None,
        acquisition_cost: None,
        current_value: None,
        depreciation_rate: None,
        warranty_expiry: None,
        metadata: None,
    }
}

/// First-time setup: create admin account. Fails if any user exists.
/// Uses an advisory lock to prevent TOCTOU race.
async fn setup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetupRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    let pw_chars = req.password.chars().count();
    if !is_valid_username(&req.username) || !(PASSWORD_MIN_LEN..=PASSWORD_MAX_LEN).contains(&pw_chars) {
        return Err(AppError::BadRequest(
            "Username must be 2–32 alphanumeric/underscore/hyphen chars; password must be 8–128 characters".into(),
        ));
    }

    let user_id = Uuid::new_v4();
    // SEC-8: Hash password BEFORE acquiring the advisory lock.
    // Argon2 is CPU-intensive (~300 ms); holding the pg advisory lock while hashing
    // serialises all concurrent callers unnecessarily.
    let pw_hash = hash_password(&req.password).await?;

    // SEC-10: Use a named, non-colliding advisory lock ID.
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(ADVISORY_LOCK_SETUP)
        .execute(&mut *tx)
        .await?;

    // Check no users exist (now safe under advisory lock)
    let count = state.user_queries.count_in_tx(&mut tx).await?;
    if count > 0 {
        return Err(AppError::Conflict("Setup already completed".into()));
    }

    // Create the admin user within the advisory-locked transaction
    let user = state.user_queries.create_in_tx(
        &mut tx, user_id, &req.username, &pw_hash, req.display_name.as_deref(), "admin",
    ).await?;

    // Create user's ephemeral container via event store (survives rebuild_all)
    let container_id = Uuid::new_v4();
    let create_req = build_user_container_request(&req.username, req.display_name.as_deref());

    let evt_metadata = EventMetadata::default();
    state.item_commands.create_item_in_tx(&mut tx, container_id, &create_req, user_id, &evt_metadata).await?;

    // Link user to their container
    state.user_queries.set_container_in_tx(&mut tx, user_id, container_id).await?;

    // Issue tokens
    let access_token = create_access_token(
        user_id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_access_ttl_secs,
    )?;

    let issued = state.token_repository.issue_refresh_token_in_tx(
        &mut tx, user_id, "setup", state.config.jwt_refresh_ttl_days, None,
    ).await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            access_token,
            refresh_token: issued.raw_token,
            expires_in: state.config.jwt_access_ttl_secs,
            user: user.into(),
        }),
    ))
}

/// Authenticate and return access + refresh tokens.
async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    // SEC-2: Prevent timing oracle — run dummy Argon2 verify when user is not found
    // so an attacker cannot distinguish "user not found" from "wrong password" by timing.
    let user_opt = state
        .user_queries
        .find_active_by_username(&req.username)
        .await?;

    const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let (user, real_hash) = match user_opt {
        Some(u) => {
            let hash = u.password_hash.clone();
            (Some(u), hash)
        }
        None => (None, DUMMY_HASH.to_string()),
    };

    let valid = verify_password(&req.password, &real_hash).await?;
    let user = match user {
        Some(u) if valid => u,
        _ => return Err(AppError::Unauthorized),
    };

    let access_token = create_access_token(
        user.id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_access_ttl_secs,
    )?;

    let issued = state.token_repository.issue_refresh_token(
        user.id,
        req.device_name.as_deref().unwrap_or("unknown"),
        state.config.jwt_refresh_ttl_days,
    ).await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: issued.raw_token,
        expires_in: state.config.jwt_access_ttl_secs,
        user: user.into(),
    }))
}

/// Rotate refresh token and issue new access token.
/// Implements reuse detection: if a previously-rotated token is presented,
/// all tokens in the same family are purged (compromised chain).
async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RefreshRequest>,
) -> AppResult<Json<AuthResponse>> {
    let token_hash = crate::auth::jwt::hash_refresh_token(&req.refresh_token);

    let mut tx = state.pool.begin().await?;

    // Try to find a valid (non-revoked, non-expired) token
    let row = state
        .token_repository
        .find_valid_by_hash_in_tx(&mut tx, &token_hash)
        .await?;

    let row = match row {
        Some(r) => r,
        None => {
            // Token not valid — check if it was previously revoked (reuse detection).
            if let Some(revoked) = state
                .token_repository
                .find_revoked_by_hash_in_tx(&mut tx, &token_hash)
                .await?
            {
                // Reuse detected! Purge the entire token family.
                tracing::warn!(
                    user_id = %revoked.user_id,
                    family_id = %revoked.family_id,
                    "Refresh token reuse detected — purging token family"
                );
                state.token_repository.purge_family_in_tx(&mut tx, revoked.family_id).await?;
                tx.commit().await?;
            }
            return Err(AppError::Unauthorized);
        }
    };

    // Revoke the used token (soft-delete for reuse detection)
    state.token_repository.revoke_by_id_in_tx(&mut tx, row.id).await?;

    let user = state
        .user_queries
        .find_active_by_id_in_tx(&mut tx, row.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let access_token = create_access_token(
        user.id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_access_ttl_secs,
    )?;

    let issued = state.token_repository.issue_refresh_token_in_tx(
        &mut tx,
        user.id,
        row.device_name.as_deref().unwrap_or("unknown"),
        state.config.jwt_refresh_ttl_days,
        Some(row.family_id), // same family chain
    ).await?;

    tx.commit().await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: issued.raw_token,
        expires_in: state.config.jwt_access_ttl_secs,
        user: user.into(),
    }))
}

/// Revoke the current refresh token.
async fn logout(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<RefreshRequest>,
) -> AppResult<StatusCode> {
    let token_hash = crate::auth::jwt::hash_refresh_token(&req.refresh_token);
    state.token_repository.revoke_by_hash(&token_hash, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Get current authenticated user profile.
async fn me(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<UserPublic>> {
    let user = state.user_queries.find_by_id(auth.user_id).await?;
    Ok(Json(user.into()))
}

/// Generate a single-use invite code (admin only).
async fn create_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<(StatusCode, Json<InviteResponse>)> {
    auth.require_role("admin")?;

    let invite = state.token_repository.create_invite(auth.user_id, 7).await?;

    Ok((
        StatusCode::CREATED,
        Json(InviteResponse {
            code: invite.code,
            expires_at: invite.expires_at,
        }),
    ))
}

/// Register a new user with a valid invite code.
/// All operations are wrapped in a single transaction for consistency.
async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    let pw_chars = req.password.chars().count();
    if !is_valid_username(&req.username) || !(PASSWORD_MIN_LEN..=PASSWORD_MAX_LEN).contains(&pw_chars) {
        return Err(AppError::BadRequest(
            "Username must be 2–32 alphanumeric/underscore/hyphen chars; password must be 8–128 characters".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;

    // Validate invite code
    let invite = state
        .token_repository
        .find_valid_invite_in_tx(&mut tx, &req.invite_code)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired invite code".into()))?;

    // Check username uniqueness
    if state.user_queries.username_exists_in_tx(&mut tx, &req.username).await? {
        return Err(AppError::Conflict("Username already taken".into()));
    }

    let user_id = Uuid::new_v4();
    let pw_hash = hash_password(&req.password).await?;

    let user = state.user_queries.create_in_tx(
        &mut tx, user_id, &req.username, &pw_hash, req.display_name.as_deref(), "member",
    ).await.map_err(|e| match &e {
        AppError::Database(sqlx::Error::Database(db_err))
            if db_err.constraint() == Some("users_username_key") =>
        {
            AppError::Conflict("Username already taken".into())
        }
        _ => e,
    })?;

    // Mark invite as used
    state.token_repository.mark_invite_used_in_tx(&mut tx, invite.id, user_id).await?;

    // Create user's ephemeral container via event store (survives rebuild_all)
    let container_id = Uuid::new_v4();
    let create_req = build_user_container_request(&req.username, req.display_name.as_deref());

    let evt_metadata = EventMetadata::default();
    state.item_commands.create_item_in_tx(&mut tx, container_id, &create_req, user_id, &evt_metadata).await?;

    state.user_queries.set_container_in_tx(&mut tx, user_id, container_id).await?;

    // Issue tokens
    let access_token = create_access_token(
        user_id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_access_ttl_secs,
    )?;

    let issued = state.token_repository.issue_refresh_token_in_tx(
        &mut tx, user_id, "registration", state.config.jwt_refresh_ttl_days, None,
    ).await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            access_token,
            refresh_token: issued.raw_token,
            expires_in: state.config.jwt_access_ttl_secs,
            user: user.into(),
        }),
    ))
}
