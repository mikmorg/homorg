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
    // Rate limiting (requests per second per IP); disabled unless RATE_LIMIT_RPS is set
    pub rate_limit_enabled: bool,
    pub rate_limit_rps: u64,
    pub rate_limit_burst: u32,
    // Request timeout
    pub request_timeout_secs: u64,
    pub upload_timeout_secs: u64,
    // Logging
    pub log_format: String, // "text" or "json"
}

/// SEC-9: Parse an env var with a fallback default.
/// Prints a warning to stderr when the var is set but cannot be parsed, so operators
/// are never silently surprised by a wrong value being replaced with a default.
fn parse_env<T>(var: &str, default: T) -> T
where
    T: std::str::FromStr + std::fmt::Display,
    T::Err: std::fmt::Display,
{
    match env::var(var) {
        Err(_) => default, // not set → use default silently
        Ok(val) => val.parse().unwrap_or_else(|e| {
            eprintln!(
                "WARNING: env var {var}='{}' could not be parsed ({}); using default {}",
                val, e, default
            );
            default
        }),
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let jwt_secret = env::var("JWT_SECRET").map_err(|e| format!("JWT_SECRET: {e}"))?;
        if jwt_secret.len() < 32 {
            return Err(format!(
                "JWT_SECRET must be at least 32 characters for adequate security (got {})",
                jwt_secret.len()
            ));
        }

        // DB-5: Validate BARCODE_PREFIX ≤ 8 chars to match the barcode_sequences column width.
        let barcode_prefix = env::var("BARCODE_PREFIX").unwrap_or_else(|_| "HOM".into());
        if barcode_prefix.len() > 8 {
            return Err(format!(
                "BARCODE_PREFIX '{}' is too long (max 8 characters, got {})",
                barcode_prefix,
                barcode_prefix.len()
            ));
        }

        let cfg = Self {
            database_url: env::var("DATABASE_URL").map_err(|e| format!("DATABASE_URL: {e}"))?,
            jwt_secret,
            jwt_access_ttl_secs: parse_env("JWT_ACCESS_TTL_SECS", 900u64),
            jwt_refresh_ttl_days: parse_env("JWT_REFRESH_TTL_DAYS", 30u64),
            listen_addr: env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            barcode_prefix,
            barcode_pad_width: parse_env("BARCODE_PAD_WIDTH", 6usize),
            storage_path: env::var("STORAGE_PATH").unwrap_or_else(|_| "./data/images".into()),
            max_batch_size: parse_env("MAX_BATCH_SIZE", 500usize),
            cors_origins: env::var("CORS_ORIGINS")
                .unwrap_or_else(|_| "*".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            // DB pool
            db_max_connections: parse_env("DB_MAX_CONNECTIONS", 20u32),
            db_min_connections: parse_env("DB_MIN_CONNECTIONS", 2u32),
            db_acquire_timeout_secs: parse_env("DB_ACQUIRE_TIMEOUT_SECS", 30u64),
            db_idle_timeout_secs: parse_env("DB_IDLE_TIMEOUT_SECS", 600u64),
            db_max_lifetime_secs: parse_env("DB_MAX_LIFETIME_SECS", 1800u64),
            // Upload limits
            max_upload_bytes: parse_env("MAX_UPLOAD_BYTES", 10_485_760usize), // 10 MiB
            allowed_image_mimes: env::var("ALLOWED_IMAGE_MIMES")
                .unwrap_or_else(|_| "image/jpeg,image/png,image/webp,image/gif".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            // Rate limiting — disabled by default; only active when RATE_LIMIT_RPS is set.
            rate_limit_enabled: env::var("RATE_LIMIT_RPS").is_ok(),
            rate_limit_rps: parse_env("RATE_LIMIT_RPS", 50u64),
            rate_limit_burst: parse_env("RATE_LIMIT_BURST", 200u32),
            // Request timeout
            request_timeout_secs: parse_env("REQUEST_TIMEOUT_SECS", 30u64),
            upload_timeout_secs: parse_env("UPLOAD_TIMEOUT_SECS", 120u64),
            // Logging
            log_format: env::var("LOG_FORMAT").unwrap_or_else(|_| "text".into()),
        };

        if cfg.max_batch_size == 0 {
            return Err("MAX_BATCH_SIZE must be at least 1".into());
        }
        if cfg.db_min_connections > cfg.db_max_connections {
            return Err(format!(
                "DB_MIN_CONNECTIONS ({}) must not exceed DB_MAX_CONNECTIONS ({})",
                cfg.db_min_connections, cfg.db_max_connections
            ));
        }

        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env vars are process-global; serialize config tests to prevent races
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Set the minimum required env vars for `from_env()` to succeed.
    fn set_required_env() {
        std::env::set_var("DATABASE_URL", "postgres://test:test@localhost/test");
        std::env::set_var("JWT_SECRET", "test-secret-that-is-at-least-32-chars-long!");
    }

    #[test]
    fn from_env_with_required_vars_uses_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
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
        std::env::remove_var("REQUEST_TIMEOUT_SECS");
        std::env::remove_var("UPLOAD_TIMEOUT_SECS");
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
        assert_eq!(config.rate_limit_rps, 50);
        assert!(!config.rate_limit_enabled);
        assert_eq!(config.request_timeout_secs, 30);
        assert_eq!(config.upload_timeout_secs, 120);
        assert_eq!(config.log_format, "text");
    }

    #[test]
    fn from_env_fails_without_database_url() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("DATABASE_URL");
        std::env::set_var("JWT_SECRET", "test-secret-that-is-at-least-32-chars-long!");
        let result = AppConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("DATABASE_URL"));
    }

    #[test]
    fn from_env_rejects_short_jwt_secret() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("DATABASE_URL", "postgres://test:test@localhost/test");
        std::env::set_var("JWT_SECRET", "too-short");
        let result = AppConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 32 characters"));
    }

    #[test]
    fn max_batch_size_zero_is_rejected() {
        let _guard = ENV_LOCK.lock().unwrap();
        set_required_env();
        std::env::set_var("MAX_BATCH_SIZE", "0");
        let result = AppConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("MAX_BATCH_SIZE"));
        std::env::remove_var("MAX_BATCH_SIZE");
    }

    #[test]
    fn db_min_greater_than_max_is_rejected() {
        let _guard = ENV_LOCK.lock().unwrap();
        set_required_env();
        std::env::set_var("DB_MIN_CONNECTIONS", "10");
        std::env::set_var("DB_MAX_CONNECTIONS", "5");
        let result = AppConfig::from_env();
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("DB_MIN_CONNECTIONS"), "unexpected: {msg}");
        assert!(msg.contains("DB_MAX_CONNECTIONS"), "unexpected: {msg}");
        std::env::remove_var("DB_MIN_CONNECTIONS");
        std::env::remove_var("DB_MAX_CONNECTIONS");
    }

    #[test]
    fn cors_origins_splits_on_comma() {
        let _guard = ENV_LOCK.lock().unwrap();
        set_required_env();
        std::env::set_var("CORS_ORIGINS", "http://a.com, http://b.com");
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.cors_origins, vec!["http://a.com", "http://b.com"]);
    }

    #[test]
    fn barcode_prefix_too_long_is_rejected() {
        let _guard = ENV_LOCK.lock().unwrap();
        set_required_env();
        // DB-5: prefix must be ≤ 8 characters
        std::env::set_var("BARCODE_PREFIX", "TOOLONGPREFIX");
        let result = AppConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("BARCODE_PREFIX"));
        std::env::remove_var("BARCODE_PREFIX");
    }
}
