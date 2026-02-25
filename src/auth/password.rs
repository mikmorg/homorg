use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};

use crate::errors::{AppError, AppResult};

/// Hash a password using Argon2id with OWASP-recommended parameters.
/// Runs in a blocking thread to avoid stalling the Tokio runtime.
pub async fn hash_password(password: &str) -> AppResult<String> {
    let password = password.to_string();
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default(); // Argon2id with default (secure) params
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(format!("Password hashing failed: {e}")))?;
        Ok(hash.to_string())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Blocking task failed: {e}")))?
}

/// Verify a password against a stored Argon2id hash.
/// Runs in a blocking thread to avoid stalling the Tokio runtime.
pub async fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let password = password.to_string();
    let hash = hash.to_string();
    tokio::task::spawn_blocking(move || {
        let parsed_hash = PasswordHash::new(&hash)
            .map_err(|e| AppError::Internal(format!("Invalid password hash: {e}")))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Blocking task failed: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hash_and_verify_correct_password() {
        let hash = hash_password("mypassword123").await.unwrap();
        assert!(verify_password("mypassword123", &hash).await.unwrap());
    }

    #[tokio::test]
    async fn verify_rejects_wrong_password() {
        let hash = hash_password("correct-password").await.unwrap();
        assert!(!verify_password("wrong-password", &hash).await.unwrap());
    }

    #[tokio::test]
    async fn hash_uses_unique_salt_each_time() {
        let h1 = hash_password("same-password").await.unwrap();
        let h2 = hash_password("same-password").await.unwrap();
        assert_ne!(h1, h2); // different salts → different hashes
    }
}
