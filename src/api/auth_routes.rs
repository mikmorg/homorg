use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::jwt::{
    create_access_token, generate_refresh_token, hash_refresh_token,
};
use crate::auth::middleware::AuthUser;
use crate::auth::password::{hash_password, verify_password};
use crate::errors::{AppError, AppResult};
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

/// First-time setup: create admin account. Fails if any user exists.
async fn setup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetupRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    // Check no users exist
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;

    if count > 0 {
        return Err(AppError::Conflict("Setup already completed".into()));
    }

    if req.username.is_empty() || req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Username required and password must be at least 8 characters".into(),
        ));
    }

    let user_id = Uuid::new_v4();
    let pw_hash = hash_password(&req.password)?;

    // Create the admin user
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, username, password_hash, display_name, role)
        VALUES ($1, $2, $3, $4, 'admin')
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(&req.username)
    .bind(&pw_hash)
    .bind(&req.display_name)
    .fetch_one(&state.pool)
    .await?;

    // Create user's ephemeral container
    let container_barcode = format!("USR-{}", req.username.to_uppercase());
    let container_label = crate::commands::item_commands::barcode_to_ltree_label(&container_barcode);
    let container_id = Uuid::new_v4();
    let container_path = format!("Root.Users.{}", container_label);

    // Well-known Users container UUID
    let users_container_id = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

    sqlx::query(
        r#"
        INSERT INTO items (id, system_barcode, ltree_label, name, is_container, container_path, parent_id, created_by)
        VALUES ($1, $2, $3, $4, TRUE, $5::ltree, $6, $7)
        "#,
    )
    .bind(container_id)
    .bind(&container_barcode)
    .bind(&container_label)
    .bind(format!("{}'s Items", req.display_name.as_deref().unwrap_or(&req.username)))
    .bind(&container_path)
    .bind(users_container_id)
    .bind(user_id)
    .execute(&state.pool)
    .await?;

    // Link user to their container
    sqlx::query("UPDATE users SET container_id = $1 WHERE id = $2")
        .bind(container_id)
        .bind(user_id)
        .execute(&state.pool)
        .await?;

    // Issue tokens
    let access_token = create_access_token(
        user_id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_access_ttl_secs,
    )?;

    let refresh_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&refresh_token);
    let expires_at = Utc::now() + Duration::days(state.config.jwt_refresh_ttl_days as i64);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (id, user_id, token_hash, device_name, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(&token_hash)
    .bind("setup")
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            access_token,
            refresh_token,
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
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE username = $1 AND is_active = TRUE",
    )
    .bind(&req.username)
    .fetch_optional(&state.pool)
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

    let refresh_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&refresh_token);
    let expires_at = Utc::now() + Duration::days(state.config.jwt_refresh_ttl_days as i64);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (id, user_id, token_hash, device_name, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(&token_hash)
    .bind(req.device_name.as_deref().unwrap_or("unknown"))
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        expires_in: state.config.jwt_access_ttl_secs,
        user: user.into(),
    }))
}

/// Rotate refresh token and issue new access token.
async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RefreshRequest>,
) -> AppResult<Json<AuthResponse>> {
    let token_hash = hash_refresh_token(&req.refresh_token);

    let row = sqlx::query_as::<_, RefreshTokenRow>(
        "SELECT * FROM refresh_tokens WHERE token_hash = $1 AND expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    // Delete the used token (rotation)
    sqlx::query("DELETE FROM refresh_tokens WHERE id = $1")
        .bind(row.id)
        .execute(&state.pool)
        .await?;

    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1 AND is_active = TRUE",
    )
    .bind(row.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let access_token = create_access_token(
        user.id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_access_ttl_secs,
    )?;

    let new_refresh = generate_refresh_token();
    let new_hash = hash_refresh_token(&new_refresh);
    let expires_at = Utc::now() + Duration::days(state.config.jwt_refresh_ttl_days as i64);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (id, user_id, token_hash, device_name, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(&new_hash)
    .bind(row.device_name.as_deref().unwrap_or("unknown"))
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: new_refresh,
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
    let token_hash = hash_refresh_token(&req.refresh_token);

    sqlx::query(
        "DELETE FROM refresh_tokens WHERE token_hash = $1 AND user_id = $2",
    )
    .bind(&token_hash)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get current authenticated user profile.
async fn me(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<UserPublic>> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1",
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    Ok(Json(user.into()))
}

/// Generate a single-use invite code (admin only).
async fn create_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<(StatusCode, Json<InviteResponse>)> {
    auth.require_role("admin")?;

    let code = generate_refresh_token(); // reuse the random string generator
    let expires_at = Utc::now() + Duration::days(7);

    sqlx::query(
        r#"
        INSERT INTO invite_tokens (id, code, created_by, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&code)
    .bind(auth.user_id)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(InviteResponse { code, expires_at }),
    ))
}

/// Register a new user with a valid invite code.
async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    if req.username.is_empty() || req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Username required and password must be at least 8 characters".into(),
        ));
    }

    // Validate invite code
    let invite = sqlx::query_as::<_, InviteToken>(
        "SELECT * FROM invite_tokens WHERE code = $1 AND used_by IS NULL AND expires_at > NOW()",
    )
    .bind(&req.invite_code)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Invalid or expired invite code".into()))?;

    // Check username uniqueness
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)",
    )
    .bind(&req.username)
    .fetch_one(&state.pool)
    .await?;

    if exists {
        return Err(AppError::Conflict("Username already taken".into()));
    }

    let user_id = Uuid::new_v4();
    let pw_hash = hash_password(&req.password)?;

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, username, password_hash, display_name, role)
        VALUES ($1, $2, $3, $4, 'member')
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(&req.username)
    .bind(&pw_hash)
    .bind(&req.display_name)
    .fetch_one(&state.pool)
    .await?;

    // Mark invite as used
    sqlx::query("UPDATE invite_tokens SET used_by = $1 WHERE id = $2")
        .bind(user_id)
        .bind(invite.id)
        .execute(&state.pool)
        .await?;

    // Create user's ephemeral container
    let container_barcode = format!("USR-{}", req.username.to_uppercase());
    let container_label = crate::commands::item_commands::barcode_to_ltree_label(&container_barcode);
    let container_id = Uuid::new_v4();
    let container_path = format!("Root.Users.{}", container_label);

    let users_container_id = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

    sqlx::query(
        r#"
        INSERT INTO items (id, system_barcode, ltree_label, name, is_container, container_path, parent_id, created_by)
        VALUES ($1, $2, $3, $4, TRUE, $5::ltree, $6, $7)
        "#,
    )
    .bind(container_id)
    .bind(&container_barcode)
    .bind(&container_label)
    .bind(format!("{}'s Items", req.display_name.as_deref().unwrap_or(&req.username)))
    .bind(&container_path)
    .bind(users_container_id)
    .bind(user_id)
    .execute(&state.pool)
    .await?;

    sqlx::query("UPDATE users SET container_id = $1 WHERE id = $2")
        .bind(container_id)
        .bind(user_id)
        .execute(&state.pool)
        .await?;

    // Issue tokens
    let access_token = create_access_token(
        user_id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_access_ttl_secs,
    )?;

    let refresh_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&refresh_token);
    let expires_at = Utc::now() + Duration::days(state.config.jwt_refresh_ttl_days as i64);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (id, user_id, token_hash, device_name, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(&token_hash)
    .bind("registration")
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            access_token,
            refresh_token,
            expires_in: state.config.jwt_access_ttl_secs,
            user: user.into(),
        }),
    ))
}
