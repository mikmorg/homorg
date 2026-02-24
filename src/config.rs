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
}

impl AppConfig {
    pub fn from_env() -> Result<Self, env::VarError> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")?,
            jwt_secret: env::var("JWT_SECRET")?,
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set the minimum required env vars for `from_env()` to succeed.
    fn set_required_env() {
        std::env::set_var("DATABASE_URL", "postgres://test:test@localhost/test");
        std::env::set_var("JWT_SECRET", "test-secret");
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

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.jwt_access_ttl_secs, 900);
        assert_eq!(config.jwt_refresh_ttl_days, 30);
        assert_eq!(config.listen_addr, "0.0.0.0:8080");
        assert_eq!(config.barcode_prefix, "HOM");
        assert_eq!(config.max_batch_size, 500);
    }

    #[test]
    fn from_env_fails_without_database_url() {
        std::env::remove_var("DATABASE_URL");
        std::env::set_var("JWT_SECRET", "test-secret");
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
