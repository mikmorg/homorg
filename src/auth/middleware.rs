use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use std::sync::Arc;

use crate::auth::jwt::{decode_access_token, Claims};
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
        let level = role_level(&self.role);
        let required = role_level(minimum);
        if level >= required {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

fn role_level(role: &str) -> u8 {
    match role {
        "admin" => 3,
        "member" => 2,
        "readonly" => 1,
        _ => 0,
    }
}

/// Axum extractor that validates JWT from Authorization header.
impl FromRequestParts<Arc<crate::AppState>> for AuthUser {
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<crate::AppState>,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let config = state.config.clone();
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        async move {
            let header = auth_header.ok_or(AppError::Unauthorized)?;
            let token = header
                .strip_prefix("Bearer ")
                .ok_or(AppError::Unauthorized)?;

            let claims: Claims = decode_access_token(token, &config.jwt_secret)?;

            Ok(AuthUser {
                user_id: claims.sub,
                role: claims.role,
            })
        }
    }
}
