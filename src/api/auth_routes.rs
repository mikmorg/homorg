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
use crate::constants::{PASSWORD_MAX_LEN, PASSWORD_MIN_LEN, USERS_ID};
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

/// Build a [`CreateItemRequest`] for a new user's personal container.
fn build_user_container_request(username: &str, display_name: Option<&str>) -> CreateItemRequest {
    let container_barcode = format!("USR-{}", username.to_uppercase());
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
    // Acquire advisory lock to prevent concurrent setup race
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(1)")
        .execute(&mut *tx)
        .await?;

    // Check no users exist (now safe under advisory lock)
    let count = state.user_queries.count_in_tx(&mut tx).await?;

    if count > 0 {
        return Err(AppError::Conflict("Setup already completed".into()));
    }

    if req.username.is_empty() || req.password.len() < PASSWORD_MIN_LEN || req.password.len() > PASSWORD_MAX_LEN {
        return Err(AppError::BadRequest(
            "Username required and password must be 8–128 characters".into(),
        ));
    }

    let user_id = Uuid::new_v4();
    let pw_hash = hash_password(&req.password)?;

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
        &mut tx, user_id, "setup", state.config.jwt_refresh_ttl_days,
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
    let user = state
        .user_queries
        .find_active_by_username(&req.username)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let valid = verify_password(&req.password, &user.password_hash)?;
    if !valid {
        return Err(AppError::Unauthorized);
    }

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
/// Delete + insert wrapped in a transaction to prevent token loss on crash.
async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RefreshRequest>,
) -> AppResult<Json<AuthResponse>> {
    let token_hash = crate::auth::jwt::hash_refresh_token(&req.refresh_token);

    let mut tx = state.pool.begin().await?;

    let row = state
        .token_repository
        .find_valid_by_hash_in_tx(&mut tx, &token_hash)
        .await?
        .ok_or(AppError::Unauthorized)?;

    // Delete the used token (rotation)
    state.token_repository.delete_by_id_in_tx(&mut tx, row.id).await?;

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
    if req.username.is_empty() || req.password.len() < PASSWORD_MIN_LEN || req.password.len() > PASSWORD_MAX_LEN {
        return Err(AppError::BadRequest(
            "Username required and password must be 8–128 characters".into(),
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
    let pw_hash = hash_password(&req.password)?;

    let user = state.user_queries.create_in_tx(
        &mut tx, user_id, &req.username, &pw_hash, req.display_name.as_deref(), "member",
    ).await?;

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
        &mut tx, user_id, "registration", state.config.jwt_refresh_ttl_days,
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
