use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::constants::JWT_AUDIENCE;
use crate::errors::{AppError, AppResult};

/// R4-C: `role` removed from Claims — the middleware fetches the authoritative
/// role from the DB on every request, so baking it into the token was unused
/// and misleading.  Existing tokens that include a `role` field still decode
/// correctly (serde ignores unknown fields).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,   // user id
    pub aud: String,  // audience
    pub iat: i64,
    pub exp: i64,
}

/// Generate a JWT access token.
pub fn create_access_token(
    user_id: Uuid,
    secret: &str,
    ttl_secs: u64,
) -> AppResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        aud: JWT_AUDIENCE.to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::seconds(ttl_secs as i64)).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to create JWT: {e}")))
}

/// Validate and decode a JWT access token.
pub fn decode_access_token(token: &str, secret: &str) -> AppResult<Claims> {
    let mut validation = Validation::default();
    validation.set_audience(&[JWT_AUDIENCE]);
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims)
}

/// Generate a cryptographically random refresh token string.
pub fn generate_refresh_token() -> String {
    let bytes: [u8; 32] = rand::random();
    hex::encode(bytes)
}

/// Hash a refresh token for storage (SHA-256).
pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "super-secret-test-key-for-jwt";

    #[test]
    fn create_and_decode_roundtrip() {
        let uid = Uuid::new_v4();
        let token = create_access_token(uid, TEST_SECRET, 300).unwrap();
        let claims = decode_access_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, uid);
        assert_eq!(claims.aud, JWT_AUDIENCE);
    }

    #[test]
    fn decode_rejects_wrong_secret() {
        let uid = Uuid::new_v4();
        let token = create_access_token(uid, TEST_SECRET, 300).unwrap();
        let result = decode_access_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn decode_rejects_expired_token() {
        let uid = Uuid::new_v4();
        // Manually craft an already-expired token
        let now = Utc::now();
        let claims = Claims {
            sub: uid,
            aud: JWT_AUDIENCE.to_string(),
            iat: (now - Duration::seconds(600)).timestamp(),
            exp: (now - Duration::seconds(300)).timestamp(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();
        let result = decode_access_token(&token, TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn refresh_token_is_64_hex_chars() {
        let token = generate_refresh_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn refresh_tokens_are_unique() {
        let a = generate_refresh_token();
        let b = generate_refresh_token();
        assert_ne!(a, b);
    }

    #[test]
    fn hash_refresh_token_is_deterministic() {
        let token = "my-refresh-token";
        let h1 = hash_refresh_token(token);
        let h2 = hash_refresh_token(token);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_refresh_token_differs_for_different_inputs() {
        let h1 = hash_refresh_token("token-a");
        let h2 = hash_refresh_token("token-b");
        assert_ne!(h1, h2);
    }
}
