use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::camera::CameraToken;
use crate::models::session::ScanSession;

/// Repository for scan-session CRUD operations.
#[derive(Clone)]
pub struct SessionRepository {
    pool: PgPool,
}

impl SessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new scan session.
    pub async fn create(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        device_id: Option<&str>,
        notes: Option<&str>,
        active_container_id: Option<Uuid>,
    ) -> AppResult<ScanSession> {
        let session = sqlx::query_as::<_, ScanSession>(
            r#"
            INSERT INTO scan_sessions (id, user_id, device_id, notes, active_container_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(session_id)
        .bind(user_id)
        .bind(device_id)
        .bind(notes)
        .bind(active_container_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(session)
    }

    /// List sessions for a user, ordered by most recent first.
    pub async fn list_for_user(&self, user_id: Uuid, limit: i64) -> AppResult<Vec<ScanSession>> {
        let sessions = sqlx::query_as::<_, ScanSession>(
            "SELECT * FROM scan_sessions WHERE user_id = $1 ORDER BY started_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(sessions)
    }

    /// Get a single session belonging to a user.
    pub async fn get_for_user(&self, session_id: Uuid, user_id: Uuid) -> AppResult<ScanSession> {
        sqlx::query_as::<_, ScanSession>("SELECT * FROM scan_sessions WHERE id = $1 AND user_id = $2")
            .bind(session_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session {session_id} not found")))
    }

    /// Get an active (not ended) session belonging to a user.
    pub async fn get_active_for_user(&self, session_id: Uuid, user_id: Uuid) -> AppResult<ScanSession> {
        sqlx::query_as::<_, ScanSession>(
            "SELECT * FROM scan_sessions WHERE id = $1 AND user_id = $2 AND ended_at IS NULL",
        )
        .bind(session_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Active session not found".into()))
    }

    /// Update session stats, active container, and active item (non-transactional).
    pub async fn update_stats(
        &self,
        session_id: Uuid,
        active_container_id: Option<Uuid>,
        active_item_id: Option<Uuid>,
        items_scanned: i32,
        items_created: i32,
        items_moved: i32,
        items_errored: i32,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE scan_sessions
            SET active_container_id = $1,
                active_item_id = $2,
                items_scanned = items_scanned + $3,
                items_created = items_created + $4,
                items_moved = items_moved + $5,
                items_errored = items_errored + $6
            WHERE id = $7
            "#,
        )
        .bind(active_container_id)
        .bind(active_item_id)
        .bind(items_scanned)
        .bind(items_created)
        .bind(items_moved)
        .bind(items_errored)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update session stats within an existing transaction.
    pub async fn update_stats_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        session_id: Uuid,
        active_container_id: Option<Uuid>,
        active_item_id: Option<Uuid>,
        items_scanned: i32,
        items_created: i32,
        items_moved: i32,
        items_errored: i32,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE scan_sessions
            SET active_container_id = $1,
                active_item_id = $2,
                items_scanned = items_scanned + $3,
                items_created = items_created + $4,
                items_moved = items_moved + $5,
                items_errored = items_errored + $6
            WHERE id = $7
            "#,
        )
        .bind(active_container_id)
        .bind(active_item_id)
        .bind(items_scanned)
        .bind(items_created)
        .bind(items_moved)
        .bind(items_errored)
        .bind(session_id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// End a session (set ended_at).
    pub async fn end_session(&self, session_id: Uuid, user_id: Uuid) -> AppResult<ScanSession> {
        sqlx::query_as::<_, ScanSession>(
            r#"
            UPDATE scan_sessions
            SET ended_at = NOW()
            WHERE id = $1 AND user_id = $2 AND ended_at IS NULL
            RETURNING *
            "#,
        )
        .bind(session_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Active session not found".into()))
    }

    /// Decrement session stats (for undo). Uses GREATEST to prevent underflow.
    ///
    /// If `session_id` is not a valid UUID the call is a no-op: the metadata
    /// field is stored as free text, so non-UUID values cannot match any
    /// `scan_sessions.id` row.
    pub async fn decrement_stats_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        session_id: &str,
        items_scanned: i32,
        items_created: i32,
        items_moved: i32,
    ) -> AppResult<()> {
        // session_id is stored as free text in event metadata; gracefully skip
        // if it cannot be parsed as a UUID (would never match a scan_sessions row).
        let Ok(session_uuid) = uuid::Uuid::parse_str(session_id) else {
            return Ok(());
        };
        sqlx::query(
            r#"
            UPDATE scan_sessions
            SET items_scanned = GREATEST(items_scanned - $1, 0),
                items_created = GREATEST(items_created - $2, 0),
                items_moved   = GREATEST(items_moved   - $3, 0)
            WHERE id = $4
            "#,
        )
        .bind(items_scanned)
        .bind(items_created)
        .bind(items_moved)
        .bind(session_uuid)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    // ── Camera token operations ─────────────────────────────────────────

    /// Create a camera token for a session.
    pub async fn create_camera_token(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        token: &str,
        device_name: Option<&str>,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<CameraToken> {
        let ct = sqlx::query_as::<_, CameraToken>(
            r#"
            INSERT INTO camera_tokens (session_id, user_id, token, device_name, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(session_id)
        .bind(user_id)
        .bind(token)
        .bind(device_name)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(ct)
    }

    /// Look up a valid (non-expired, non-revoked) camera token.
    pub async fn get_camera_token(&self, token: &str) -> AppResult<CameraToken> {
        sqlx::query_as::<_, CameraToken>(
            r#"
            SELECT * FROM camera_tokens
            WHERE token = $1
              AND revoked_at IS NULL
              AND expires_at > NOW()
            "#,
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::Unauthorized)
    }

    /// List active camera tokens for a session.
    pub async fn list_camera_tokens(&self, session_id: Uuid) -> AppResult<Vec<CameraToken>> {
        let tokens = sqlx::query_as::<_, CameraToken>(
            r#"
            SELECT * FROM camera_tokens
            WHERE session_id = $1
              AND revoked_at IS NULL
              AND expires_at > NOW()
            ORDER BY created_at DESC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(tokens)
    }

    /// Revoke a camera token.
    pub async fn revoke_camera_token(&self, token_id: Uuid, user_id: Uuid) -> AppResult<()> {
        let rows = sqlx::query(
            r#"
            UPDATE camera_tokens
            SET revoked_at = NOW()
            WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
        )
        .bind(token_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if rows == 0 {
            return Err(AppError::NotFound("Camera token not found".into()));
        }
        Ok(())
    }

    /// Revoke all camera tokens for a session.
    pub async fn revoke_all_camera_tokens(&self, session_id: Uuid, user_id: Uuid) -> AppResult<u64> {
        let rows = sqlx::query(
            r#"
            UPDATE camera_tokens
            SET revoked_at = NOW()
            WHERE session_id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
        )
        .bind(session_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(rows)
    }

    /// R4-A: End sessions that have been open longer than `idle_hours` hours and revoke
    /// their camera tokens.  Returns the number of sessions closed.
    pub async fn cleanup_stale_sessions(&self, idle_hours: i64) -> AppResult<u64> {
        // Close stale sessions and collect their IDs in one statement.
        let stale_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE scan_sessions
            SET ended_at = NOW()
            WHERE ended_at IS NULL
              AND started_at < NOW() - make_interval(hours => $1)
            RETURNING id
            "#,
        )
        .bind(idle_hours)
        .fetch_all(&self.pool)
        .await?;

        let count = stale_ids.len() as u64;

        if !stale_ids.is_empty() {
            // Revoke camera tokens for closed sessions.
            sqlx::query(
                "UPDATE camera_tokens SET revoked_at = NOW() \
                 WHERE session_id = ANY($1) AND revoked_at IS NULL",
            )
            .bind(&stale_ids)
            .execute(&self.pool)
            .await?;
        }

        Ok(count)
    }

    /// Get an active session by ID (for camera token validation — no user check).
    pub async fn get_session_by_id(&self, session_id: Uuid) -> AppResult<ScanSession> {
        sqlx::query_as::<_, ScanSession>("SELECT * FROM scan_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session {session_id} not found")))
    }
}
