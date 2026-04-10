use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::errors::{AppError, AppResult};

/// Trait for pluggable storage backends (local filesystem, S3, etc.)
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn upload(&self, item_id: Uuid, filename: &str, data: &[u8]) -> AppResult<String>;
    async fn delete(&self, key: &str) -> AppResult<()>;
    fn get_url(&self, key: &str) -> String;
}

/// Local filesystem storage backend for Phase 1.
pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: PathBuf::from(base_path),
        }
    }

    /// Ensure the storage directory exists.
    pub async fn init(&self) -> AppResult<()> {
        fs::create_dir_all(&self.base_path)
            .await
            .map_err(|e| AppError::Storage(format!("Failed to create storage directory: {e}")))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageBackend for LocalStorage {
    async fn upload(&self, item_id: Uuid, filename: &str, data: &[u8]) -> AppResult<String> {
        let dir = self.base_path.join(item_id.to_string());
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| AppError::Storage(format!("Failed to create item directory: {e}")))?;

        let file_id = Uuid::new_v4();
        let raw_ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
            .to_ascii_lowercase();
        // ST-1: Only allow known image extensions to prevent content-type confusion
        // if the storage directory is ever served directly by a reverse proxy.
        let ext = match raw_ext.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "svg" | "bmp" | "tiff" | "tif" => {
                &raw_ext
            }
            _ => "bin",
        };
        let storage_filename = format!("{file_id}.{ext}");
        let file_path = dir.join(&storage_filename);

        // RM-2: Atomic write — write to a temp file then rename so a crash during
        // write never leaves a partial/corrupt file at the final path.
        let tmp_path = file_path.with_extension(format!("{ext}.tmp"));
        fs::write(&tmp_path, data)
            .await
            .map_err(|e| AppError::Storage(format!("Failed to write temp file: {e}")))?;
        fs::rename(&tmp_path, &file_path).await.map_err(|e| {
            // Best-effort cleanup of the temp file; ignore secondary errors.
            let _ = std::fs::remove_file(&tmp_path);
            AppError::Storage(format!("Failed to finalize file (rename): {e}"))
        })?;

        let key = format!("{}/{}", item_id, storage_filename);
        Ok(key)
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        // CB-8: The stored image path is the URL form "/files/{item_id}/{file}".
        // storage.delete() is called with this URL path, but our base_path is the
        // storage root (e.g. "./data/images").  PathBuf::join with an absolute path
        // replaces the base entirely, so canonicalize() then fails and images are
        // never actually removed from disk.  Strip the known URL prefix so we
        // always operate on the relative key no matter how it was stored.
        let stripped = key.strip_prefix("/files/").unwrap_or(key);
        let file_path = self.base_path.join(stripped);
        // Prevent path traversal: canonicalize and verify within base
        let canonical = file_path
            .canonicalize()
            .map_err(|_| AppError::Storage("Invalid storage key".into()))?;
        let base_canonical = self
            .base_path
            .canonicalize()
            .map_err(|e| AppError::Storage(format!("Storage base path error: {e}")))?;
        if !canonical.starts_with(&base_canonical) {
            return Err(AppError::BadRequest("Invalid storage key".into()));
        }
        if canonical.exists() {
            fs::remove_file(&canonical)
                .await
                .map_err(|e| AppError::Storage(format!("Failed to delete file: {e}")))?;
        }
        Ok(())
    }

    fn get_url(&self, key: &str) -> String {
        // Reject any key containing path traversal sequences
        let safe_key = if key.contains("..") {
            key.replace(['/', '\\'], "_").replace("..", "")
        } else {
            key.to_string()
        };
        format!("/files/{safe_key}")
    }
}

// ── S3-compatible storage backend (requires `s3` feature) ─────────────

#[cfg(feature = "s3")]
pub struct S3Storage {
    bucket: s3::Bucket,
    prefix: String,
    /// Public base URL for constructing image URLs (e.g. CDN or S3 endpoint).
    public_base_url: String,
}

#[cfg(feature = "s3")]
impl S3Storage {
    pub fn new(config: &AppConfig) -> Result<Self, String> {
        if config.s3_bucket.is_empty() {
            return Err("S3_BUCKET is required when STORAGE_BACKEND=s3".into());
        }

        let region = if let Some(ref endpoint) = config.s3_endpoint {
            s3::Region::Custom {
                region: config.s3_region.clone(),
                endpoint: endpoint.clone(),
            }
        } else {
            config.s3_region.parse().map_err(|e| format!("Invalid S3_REGION: {e}"))?
        };

        let credentials = s3::creds::Credentials::from_env()
            .map_err(|e| format!("Failed to load S3 credentials: {e}"))?;

        let bucket = s3::Bucket::new(&config.s3_bucket, region, credentials)
            .map_err(|e| format!("Failed to create S3 bucket handle: {e}"))?;

        let public_base_url = config
            .s3_endpoint
            .as_deref()
            .map(|ep| format!("{}/{}", ep.trim_end_matches('/'), config.s3_bucket))
            .unwrap_or_else(|| {
                format!(
                    "https://{}.s3.{}.amazonaws.com",
                    config.s3_bucket, config.s3_region
                )
            });

        Ok(Self {
            bucket,
            prefix: config.s3_prefix.clone(),
            public_base_url,
        })
    }
}

#[cfg(feature = "s3")]
#[async_trait::async_trait]
impl StorageBackend for S3Storage {
    async fn upload(&self, item_id: Uuid, filename: &str, data: &[u8]) -> AppResult<String> {
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        let file_id = Uuid::new_v4();
        let key = format!("{}/{item_id}/{file_id}.{ext}", self.prefix);

        self.bucket
            .put_object(&key, data)
            .await
            .map_err(|e| AppError::Storage(format!("S3 upload failed: {e}")))?;

        Ok(key)
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        let stripped = key.strip_prefix('/').unwrap_or(key);
        self.bucket
            .delete_object(stripped)
            .await
            .map_err(|e| AppError::Storage(format!("S3 delete failed: {e}")))?;
        Ok(())
    }

    fn get_url(&self, key: &str) -> String {
        let clean_key = key.strip_prefix('/').unwrap_or(key);
        format!("{}/{clean_key}", self.public_base_url)
    }
}

// ── Factory ───────────────────────────────────────────────────────────

/// Create the appropriate storage backend based on configuration.
pub async fn create_storage(config: &AppConfig) -> Result<std::sync::Arc<dyn StorageBackend>, String> {
    match config.storage_backend.as_str() {
        #[cfg(feature = "s3")]
        "s3" => {
            tracing::info!("Using S3 storage backend (bucket={})", config.s3_bucket);
            let s3 = S3Storage::new(config)?;
            Ok(std::sync::Arc::new(s3))
        }
        #[cfg(not(feature = "s3"))]
        "s3" => Err("S3 storage backend requires the 's3' feature flag. Rebuild with --features s3".into()),
        "local" | _ => {
            let local = LocalStorage::new(&config.storage_path);
            local.init().await.map_err(|e| format!("Failed to initialize local storage: {e}"))?;
            tracing::info!("Using local storage backend (path={})", config.storage_path);
            Ok(std::sync::Arc::new(local))
        }
    }
}
