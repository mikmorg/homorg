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
