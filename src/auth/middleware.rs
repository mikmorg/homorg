use axum::{extract::FromRequestParts, http::request::Parts};
use std::sync::Arc;

use crate::auth::jwt::{decode_access_token, Claims};
use crate::constants::Role;
use crate::errors::AppError;

/// Extracted from request: authenticated user info.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub role: String,
}

impl AuthUser {
    /// Check if user has at least the given role level.
    pub fn require_role(&self, minimum: &str) -> Result<(), AppError> {
        let level = Role::from_str_lossy(&self.role).level();
        let required = Role::from_str_lossy(minimum).level();
        if level >= required {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

/// Axum extractor that validates JWT from Authorization header
/// and verifies the user is still active in the database.
impl FromRequestParts<Arc<crate::AppState>> for AuthUser {
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<crate::AppState>,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let config = state.config.clone();
        let pool = state.pool.clone();
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        async move {
            let header = auth_header.ok_or(AppError::Unauthorized)?;
            let token = header.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;

            let claims: Claims = decode_access_token(token, &config.jwt_secret)?;

            // Verify user is still active and fetch authoritative role from DB
            let row: Option<(bool, String)> = sqlx::query_as("SELECT is_active, role FROM users WHERE id = $1")
                .bind(claims.sub)
                .fetch_optional(&pool)
                .await
                .map_err(AppError::Database)?;

            match row {
                Some((true, role)) => Ok(AuthUser {
                    user_id: claims.sub,
                    role,
                }),
                _ => Err(AppError::Unauthorized),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: &str) -> AuthUser {
        AuthUser {
            user_id: uuid::Uuid::nil(),
            role: role.to_string(),
        }
    }

    #[test]
    fn admin_can_access_admin_route() {
        assert!(user("admin").require_role("admin").is_ok());
    }

    #[test]
    fn admin_can_access_member_route() {
        assert!(user("admin").require_role("member").is_ok());
    }

    #[test]
    fn member_cannot_access_admin_route() {
        assert!(user("member").require_role("admin").is_err());
    }

    #[test]
    fn member_can_access_readonly_route() {
        assert!(user("member").require_role("readonly").is_ok());
    }

    #[test]
    fn readonly_cannot_access_member_route() {
        assert!(user("readonly").require_role("member").is_err());
    }

    #[test]
    fn unknown_role_treated_as_readonly() {
        // unknown role should be able to access readonly routes
        assert!(user("guest").require_role("readonly").is_ok());
        // but not member routes
        assert!(user("guest").require_role("member").is_err());
    }
}
