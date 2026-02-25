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
    /// If `family_id` is `None`, a new family is created (fresh login).
    /// If `Some`, the token joins an existing family (rotation).
    pub async fn issue_refresh_token_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        device_name: &str,
        ttl_days: u64,
        family_id: Option<Uuid>,
    ) -> AppResult<IssuedRefreshToken> {
        let raw = generate_refresh_token();
        let hash = hash_refresh_token(&raw);
        let expires_at = Utc::now() + Duration::days(ttl_days as i64);
        let fam = family_id.unwrap_or_else(Uuid::new_v4);

        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, device_name, expires_at, family_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(&hash)
        .bind(device_name)
        .bind(expires_at)
        .bind(fam)
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
            INSERT INTO refresh_tokens (id, user_id, token_hash, device_name, expires_at, family_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(&hash)
        .bind(device_name)
        .bind(expires_at)
        .bind(Uuid::new_v4()) // new family for non-rotation issuance
        .execute(&self.pool)
        .await?;

        Ok(IssuedRefreshToken { raw_token: raw })
    }

    /// Look up a valid (non-expired, non-revoked) refresh token by its hash within a transaction.
    pub async fn find_valid_by_hash_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        token_hash: &str,
    ) -> AppResult<Option<RefreshTokenRow>> {
        let row = sqlx::query_as::<_, RefreshTokenRow>(
            "SELECT * FROM refresh_tokens WHERE token_hash = $1 AND expires_at > NOW() AND revoked_at IS NULL FOR UPDATE",
        )
        .bind(token_hash)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(row)
    }

    /// Check if a token hash belongs to a previously-revoked token (reuse detection).
    /// Returns the family_id and user_id if found.
    pub async fn find_revoked_by_hash_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        token_hash: &str,
    ) -> AppResult<Option<RefreshTokenRow>> {
        let row = sqlx::query_as::<_, RefreshTokenRow>(
            "SELECT * FROM refresh_tokens WHERE token_hash = $1 AND revoked_at IS NOT NULL",
        )
        .bind(token_hash)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(row)
    }

    /// Mark a refresh token as revoked (soft-delete for reuse detection).
    pub async fn revoke_by_id_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// Purge an entire token family (nuclear option on reuse detection).
    pub async fn purge_family_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        family_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("DELETE FROM refresh_tokens WHERE family_id = $1")
            .bind(family_id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// Revoke a specific refresh token by hash and user (soft-revoke for reuse detection).
    pub async fn revoke_by_hash(&self, token_hash: &str, user_id: Uuid) -> AppResult<()> {
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = NOW() WHERE token_hash = $1 AND user_id = $2 AND revoked_at IS NULL",
        )
        .bind(token_hash)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Revoke all refresh tokens for a user (e.g. on deactivation). Soft-revoke preserves reuse detection.
    pub async fn revoke_all_for_user(&self, user_id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Purge revoked tokens older than the given age (housekeeping).
    pub async fn purge_stale_revoked(&self, max_age_days: i64) -> AppResult<u64> {
        let result = sqlx::query(
            "DELETE FROM refresh_tokens WHERE revoked_at IS NOT NULL AND revoked_at < NOW() - $1::interval",
        )
        .bind(format!("{max_age_days} days"))
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Purge expired tokens (past expires_at) regardless of revoked status.
    pub async fn purge_expired(&self) -> AppResult<u64> {
        let result = sqlx::query(
            "DELETE FROM refresh_tokens WHERE expires_at < NOW()",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
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

    /// Find a valid invite within a transaction (locks row for update).
    pub async fn find_valid_invite_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        code: &str,
    ) -> AppResult<Option<InviteToken>> {
        let invite = sqlx::query_as::<_, InviteToken>(
            "SELECT * FROM invite_tokens WHERE code = $1 AND used_by IS NULL AND expires_at > NOW() FOR UPDATE",
        )
        .bind(code)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(invite)
    }

    /// Mark an invite as used (conditional: only if not already used).
    pub async fn mark_invite_used_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        invite_id: Uuid,
        used_by: Uuid,
    ) -> AppResult<()> {
        let result = sqlx::query("UPDATE invite_tokens SET used_by = $1 WHERE id = $2 AND used_by IS NULL")
            .bind(used_by)
            .bind(invite_id)
            .execute(&mut **tx)
            .await?;
        if result.rows_affected() == 0 {
            return Err(crate::errors::AppError::Conflict("Invite code already used".into()));
        }
        Ok(())
    }
}
