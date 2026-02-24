use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::user::{User, UserPublic};

/// Read/write query handler for user data.
#[derive(Clone)]
pub struct UserQueries {
    pool: PgPool,
}

impl UserQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Count all users in the database.
    pub async fn count(&self) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Count active users.
    pub async fn count_active(&self) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_active = TRUE")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Count all users within a transaction (for advisory-locked setup).
    pub async fn count_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&mut **tx)
            .await?;
        Ok(count)
    }

    /// Find a user by ID.
    pub async fn find_by_id(&self, id: Uuid) -> AppResult<User> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("User {id} not found")))
    }

    /// Find an active user by username (for login).
    pub async fn find_active_by_username(&self, username: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE username = $1 AND is_active = TRUE",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    /// Find an active user by ID within a transaction.
    pub async fn find_active_by_id_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = $1 AND is_active = TRUE",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(user)
    }

    /// Check if a username already exists.
    pub async fn username_exists(&self, username: &str) -> AppResult<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)",
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    /// Check if a username exists within a transaction.
    pub async fn username_exists_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        username: &str,
    ) -> AppResult<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)",
        )
        .bind(username)
        .fetch_one(&mut **tx)
        .await?;
        Ok(exists)
    }

    /// Create a new user within a transaction.
    pub async fn create_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        username: &str,
        password_hash: &str,
        display_name: Option<&str>,
        role: &str,
    ) -> AppResult<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, username, password_hash, display_name, role)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(username)
        .bind(password_hash)
        .bind(display_name)
        .bind(role)
        .fetch_one(&mut **tx)
        .await?;
        Ok(user)
    }

    /// Create a new user (non-transactional).
    pub async fn create(
        &self,
        id: Uuid,
        username: &str,
        password_hash: &str,
        display_name: Option<&str>,
        role: &str,
    ) -> AppResult<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, username, password_hash, display_name, role)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(username)
        .bind(password_hash)
        .bind(display_name)
        .bind(role)
        .fetch_one(&self.pool)
        .await?;
        Ok(user)
    }

    /// Link a user to their personal container.
    pub async fn set_container_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        container_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("UPDATE users SET container_id = $1 WHERE id = $2")
            .bind(container_id)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// Link a user to their personal container (non-transactional).
    pub async fn set_container(&self, user_id: Uuid, container_id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE users SET container_id = $1 WHERE id = $2")
            .bind(container_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// List all users ordered by creation date.
    pub async fn list_all(&self) -> AppResult<Vec<UserPublic>> {
        let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;
        Ok(users.into_iter().map(Into::into).collect())
    }

    /// Update a user's display name.
    pub async fn update_display_name(&self, id: Uuid, display_name: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET display_name = $1 WHERE id = $2")
            .bind(display_name)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update a user's password hash.
    pub async fn update_password(&self, id: Uuid, password_hash: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(password_hash)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update a user's role.
    pub async fn update_role(&self, id: Uuid, role: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
            .bind(role)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Deactivate a user.
    pub async fn deactivate(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
