use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::jwt::{generate_refresh_token, hash_refresh_token};
use crate::errors::AppResult;
use crate::models::user::{InviteToken, RefreshTokenRow};

/// Repository for refresh-token and invite-token operations.
#[derive(Clone)]
pub struct TokenRepository {
    pool: PgPool,
}

/// Issued refresh token (raw token + hash for insertion).
pub struct IssuedRefreshToken {
    pub raw_token: String,
}

impl TokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Refresh tokens ──────────────────────────────────────────────────

    /// Issue a new refresh token within a transaction. Returns the raw (unhashed) token.
    pub async fn issue_refresh_token_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        device_name: &str,
        ttl_days: u64,
    ) -> AppResult<IssuedRefreshToken> {
        let raw = generate_refresh_token();
        let hash = hash_refresh_token(&raw);
        let expires_at = Utc::now() + Duration::days(ttl_days as i64);

        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, device_name, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(&hash)
        .bind(device_name)
        .bind(expires_at)
        .execute(&mut **tx)
        .await?;

        Ok(IssuedRefreshToken { raw_token: raw })
    }

    /// Issue a new refresh token (non-transactional). Returns the raw (unhashed) token.
    pub async fn issue_refresh_token(
        &self,
        user_id: Uuid,
        device_name: &str,
        ttl_days: u64,
    ) -> AppResult<IssuedRefreshToken> {
        let raw = generate_refresh_token();
        let hash = hash_refresh_token(&raw);
        let expires_at = Utc::now() + Duration::days(ttl_days as i64);

        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, device_name, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(&hash)
        .bind(device_name)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(IssuedRefreshToken { raw_token: raw })
    }

    /// Look up a valid (non-expired) refresh token by its hash within a transaction.
    pub async fn find_valid_by_hash_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        token_hash: &str,
    ) -> AppResult<Option<RefreshTokenRow>> {
        let row = sqlx::query_as::<_, RefreshTokenRow>(
            "SELECT * FROM refresh_tokens WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(row)
    }

    /// Delete a specific refresh token by ID within a transaction (for rotation).
    pub async fn delete_by_id_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("DELETE FROM refresh_tokens WHERE id = $1")
            .bind(id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// Revoke a specific refresh token by hash and user.
    pub async fn revoke_by_hash(&self, token_hash: &str, user_id: Uuid) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM refresh_tokens WHERE token_hash = $1 AND user_id = $2",
        )
        .bind(token_hash)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Revoke all refresh tokens for a user (e.g. on deactivation).
    pub async fn revoke_all_for_user(&self, user_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Invite tokens ───────────────────────────────────────────────────

    /// Create a new invite code.
    pub async fn create_invite(
        &self,
        created_by: Uuid,
        ttl_days: i64,
    ) -> AppResult<InviteToken> {
        let code = generate_refresh_token(); // reuse the random string generator
        let expires_at = Utc::now() + Duration::days(ttl_days);

        let invite = sqlx::query_as::<_, InviteToken>(
            r#"
            INSERT INTO invite_tokens (id, code, created_by, expires_at)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(&code)
        .bind(created_by)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(invite)
    }

    /// Find a valid (unused, non-expired) invite by code.
    pub async fn find_valid_invite(&self, code: &str) -> AppResult<Option<InviteToken>> {
        let invite = sqlx::query_as::<_, InviteToken>(
            "SELECT * FROM invite_tokens WHERE code = $1 AND used_by IS NULL AND expires_at > NOW()",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;
        Ok(invite)
    }

    /// Find a valid invite within a transaction.
    pub async fn find_valid_invite_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        code: &str,
    ) -> AppResult<Option<InviteToken>> {
        let invite = sqlx::query_as::<_, InviteToken>(
            "SELECT * FROM invite_tokens WHERE code = $1 AND used_by IS NULL AND expires_at > NOW()",
        )
        .bind(code)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(invite)
    }

    /// Mark an invite as used.
    pub async fn mark_invite_used_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        invite_id: Uuid,
        used_by: Uuid,
    ) -> AppResult<()> {
        sqlx::query("UPDATE invite_tokens SET used_by = $1 WHERE id = $2")
            .bind(used_by)
            .bind(invite_id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}
