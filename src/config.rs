use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_access_ttl_secs: u64,
    pub jwt_refresh_ttl_days: u64,
    pub listen_addr: String,
    pub barcode_prefix: String,
    pub barcode_pad_width: usize,
    pub storage_path: String,
    pub max_batch_size: usize,
    pub cors_origins: Vec<String>,
    // DB pool tuning
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_secs: u64,
    pub db_idle_timeout_secs: u64,
    pub db_max_lifetime_secs: u64,
    // Upload limits
    pub max_upload_bytes: usize,
    pub allowed_image_mimes: Vec<String>,
    // Rate limiting (requests per second per IP)
    pub rate_limit_rps: u64,
    pub rate_limit_burst: u32,
    // Logging
    pub log_format: String, // "text" or "json"
}

impl AppConfig {
    pub fn from_env() -> Result<Self, env::VarError> {
        let jwt_secret = env::var("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            eprintln!("WARNING: JWT_SECRET should be at least 32 characters for adequate security");
        }

        Ok(Self {
            database_url: env::var("DATABASE_URL")?,
            jwt_secret,
            jwt_access_ttl_secs: env::var("JWT_ACCESS_TTL_SECS")
                .unwrap_or_else(|_| "900".into())
                .parse()
                .unwrap_or(900),
            jwt_refresh_ttl_days: env::var("JWT_REFRESH_TTL_DAYS")
                .unwrap_or_else(|_| "30".into())
                .parse()
                .unwrap_or(30),
            listen_addr: env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            barcode_prefix: env::var("BARCODE_PREFIX").unwrap_or_else(|_| "HOM".into()),
            barcode_pad_width: env::var("BARCODE_PAD_WIDTH")
                .unwrap_or_else(|_| "6".into())
                .parse()
                .unwrap_or(6),
            storage_path: env::var("STORAGE_PATH").unwrap_or_else(|_| "./data/images".into()),
            max_batch_size: env::var("MAX_BATCH_SIZE")
                .unwrap_or_else(|_| "500".into())
                .parse()
                .unwrap_or(500),
            cors_origins: env::var("CORS_ORIGINS")
                .unwrap_or_else(|_| "*".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            // DB pool
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "20".into())
                .parse()
                .unwrap_or(20),
            db_min_connections: env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "2".into())
                .parse()
                .unwrap_or(2),
            db_acquire_timeout_secs: env::var("DB_ACQUIRE_TIMEOUT_SECS")
                .unwrap_or_else(|_| "30".into())
                .parse()
                .unwrap_or(30),
            db_idle_timeout_secs: env::var("DB_IDLE_TIMEOUT_SECS")
                .unwrap_or_else(|_| "600".into())
                .parse()
                .unwrap_or(600),
            db_max_lifetime_secs: env::var("DB_MAX_LIFETIME_SECS")
                .unwrap_or_else(|_| "1800".into())
                .parse()
                .unwrap_or(1800),
            // Upload limits
            max_upload_bytes: env::var("MAX_UPLOAD_BYTES")
                .unwrap_or_else(|_| "10485760".into()) // 10 MiB
                .parse()
                .unwrap_or(10_485_760),
            allowed_image_mimes: env::var("ALLOWED_IMAGE_MIMES")
                .unwrap_or_else(|_| "image/jpeg,image/png,image/webp,image/gif".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            // Rate limiting
            rate_limit_rps: env::var("RATE_LIMIT_RPS")
                .unwrap_or_else(|_| "10".into())
                .parse()
                .unwrap_or(10),
            rate_limit_burst: env::var("RATE_LIMIT_BURST")
                .unwrap_or_else(|_| "30".into())
                .parse()
                .unwrap_or(30),
            // Logging
            log_format: env::var("LOG_FORMAT").unwrap_or_else(|_| "text".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set the minimum required env vars for `from_env()` to succeed.
    fn set_required_env() {
        std::env::set_var("DATABASE_URL", "postgres://test:test@localhost/test");
        std::env::set_var("JWT_SECRET", "test-secret-that-is-at-least-32-chars-long!");
    }

    #[test]
    fn from_env_with_required_vars_uses_defaults() {
        set_required_env();
        // Remove optional vars to test defaults
        std::env::remove_var("JWT_ACCESS_TTL_SECS");
        std::env::remove_var("JWT_REFRESH_TTL_DAYS");
        std::env::remove_var("LISTEN_ADDR");
        std::env::remove_var("BARCODE_PREFIX");
        std::env::remove_var("STORAGE_PATH");
        std::env::remove_var("MAX_BATCH_SIZE");
        std::env::remove_var("CORS_ORIGINS");
        std::env::remove_var("DB_MAX_CONNECTIONS");
        std::env::remove_var("DB_MIN_CONNECTIONS");
        std::env::remove_var("DB_ACQUIRE_TIMEOUT_SECS");
        std::env::remove_var("DB_IDLE_TIMEOUT_SECS");
        std::env::remove_var("DB_MAX_LIFETIME_SECS");
        std::env::remove_var("MAX_UPLOAD_BYTES");
        std::env::remove_var("ALLOWED_IMAGE_MIMES");
        std::env::remove_var("RATE_LIMIT_RPS");
        std::env::remove_var("RATE_LIMIT_BURST");
        std::env::remove_var("LOG_FORMAT");

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.jwt_access_ttl_secs, 900);
        assert_eq!(config.jwt_refresh_ttl_days, 30);
        assert_eq!(config.listen_addr, "0.0.0.0:8080");
        assert_eq!(config.barcode_prefix, "HOM");
        assert_eq!(config.max_batch_size, 500);
        assert_eq!(config.db_max_connections, 20);
        assert_eq!(config.db_min_connections, 2);
        assert_eq!(config.max_upload_bytes, 10_485_760);
        assert_eq!(config.rate_limit_rps, 10);
        assert_eq!(config.log_format, "text");
    }

    #[test]
    fn from_env_fails_without_database_url() {
        std::env::remove_var("DATABASE_URL");
        std::env::set_var("JWT_SECRET", "test-secret-that-is-at-least-32-chars-long!");
        let result = AppConfig::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn cors_origins_splits_on_comma() {
        set_required_env();
        std::env::set_var("CORS_ORIGINS", "http://a.com, http://b.com");
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.cors_origins, vec!["http://a.com", "http://b.com"]);
    }
}
