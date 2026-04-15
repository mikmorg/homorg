use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::errors::{AppError, AppResult};

/// Normalize a file extension to a known image type, or "bin" for unknown.
/// Used by all storage backends to prevent content-type confusion.
fn sanitize_extension(filename: &str) -> &'static str {
    let raw = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    match raw.as_str() {
        "jpg" | "jpeg" => "jpg",
        "png" => "png",
        "gif" => "gif",
        "webp" => "webp",
        "avif" => "avif",
        "heic" => "heic",
        "heif" => "heif",
        "svg" => "svg",
        "bmp" => "bmp",
        "tiff" | "tif" => "tiff",
        _ => "bin",
    }
}

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
        let ext = sanitize_extension(filename);
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
            config
                .s3_region
                .parse()
                .map_err(|e| format!("Invalid S3_REGION: {e}"))?
        };

        let credentials =
            s3::creds::Credentials::from_env().map_err(|e| format!("Failed to load S3 credentials: {e}"))?;

        let bucket = s3::Bucket::new(&config.s3_bucket, region, credentials)
            .map_err(|e| format!("Failed to create S3 bucket handle: {e}"))?;

        let public_base_url = config
            .s3_endpoint
            .as_deref()
            .map(|ep| format!("{}/{}", ep.trim_end_matches('/'), config.s3_bucket))
            .unwrap_or_else(|| format!("https://{}.s3.{}.amazonaws.com", config.s3_bucket, config.s3_region));

        Ok(Self {
            bucket: *bucket,
            prefix: config.s3_prefix.clone(),
            public_base_url,
        })
    }
}

#[cfg(feature = "s3")]
#[async_trait::async_trait]
impl StorageBackend for S3Storage {
    async fn upload(&self, item_id: Uuid, filename: &str, data: &[u8]) -> AppResult<String> {
        let ext = sanitize_extension(filename);
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
        backend => {
            if backend != "local" {
                tracing::warn!("Unknown STORAGE_BACKEND '{backend}', falling back to local");
            }
            let local = LocalStorage::new(&config.storage_path);
            local
                .init()
                .await
                .map_err(|e| format!("Failed to initialize local storage: {e}"))?;
            tracing::info!("Using local storage backend (path={})", config.storage_path);
            Ok(std::sync::Arc::new(local))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── get_url tests ──────────────────────────────────────────────────

    #[test]
    fn get_url_normal_key() {
        let storage = LocalStorage::new("/data/images");
        assert_eq!(storage.get_url("abc/def.jpg"), "/files/abc/def.jpg");
    }

    #[test]
    fn get_url_sanitizes_path_traversal() {
        let storage = LocalStorage::new("/data/images");
        let url = storage.get_url("../../etc/passwd");
        assert!(!url.contains(".."), "should strip path traversal: {url}");
        assert!(url.starts_with("/files/"));
    }

    #[test]
    fn get_url_sanitizes_backslash_traversal() {
        let storage = LocalStorage::new("/data/images");
        let url = storage.get_url("..\\..\\etc\\passwd");
        assert!(!url.contains(".."), "should strip traversal: {url}");
    }

    #[test]
    fn get_url_passthrough_clean_key() {
        let storage = LocalStorage::new("/data/images");
        let key = "item-uuid/file-uuid.jpg";
        assert_eq!(storage.get_url(key), "/files/item-uuid/file-uuid.jpg");
    }

    // ── upload tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn upload_creates_file_with_allowed_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());
        let item_id = Uuid::new_v4();
        let data = b"fake image data";

        let key = storage.upload(item_id, "photo.jpg", data).await.unwrap();
        assert!(key.starts_with(&item_id.to_string()));
        assert!(key.ends_with(".jpg"));

        // Verify file exists on disk
        let full_path = tmp.path().join(&key);
        assert!(full_path.exists());
        assert_eq!(std::fs::read(&full_path).unwrap(), data);
    }

    #[tokio::test]
    async fn upload_rejects_unknown_extension_to_bin() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());
        let item_id = Uuid::new_v4();

        let key = storage.upload(item_id, "script.exe", b"data").await.unwrap();
        assert!(key.ends_with(".bin"), "unknown ext should map to .bin: {key}");
    }

    #[tokio::test]
    async fn upload_allows_png_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());
        let key = storage.upload(Uuid::new_v4(), "image.PNG", b"data").await.unwrap();
        assert!(key.ends_with(".png"), "should lowercase extension: {key}");
    }

    #[tokio::test]
    async fn upload_allows_webp_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());
        let key = storage.upload(Uuid::new_v4(), "photo.webp", b"data").await.unwrap();
        assert!(key.ends_with(".webp"));
    }

    #[tokio::test]
    async fn upload_no_extension_maps_to_bin() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());
        let key = storage.upload(Uuid::new_v4(), "noext", b"data").await.unwrap();
        assert!(key.ends_with(".bin"));
    }

    // ── delete tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_removes_uploaded_file() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());
        let item_id = Uuid::new_v4();

        let key = storage.upload(item_id, "photo.jpg", b"data").await.unwrap();
        let full_path = tmp.path().join(&key);
        assert!(full_path.exists());

        storage.delete(&key).await.unwrap();
        assert!(!full_path.exists());
    }

    #[tokio::test]
    async fn delete_strips_files_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());
        let item_id = Uuid::new_v4();

        let key = storage.upload(item_id, "photo.jpg", b"data").await.unwrap();
        let url_key = format!("/files/{key}");

        storage.delete(&url_key).await.unwrap();
        assert!(!tmp.path().join(&key).exists());
    }

    #[tokio::test]
    async fn delete_rejects_path_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());

        // Create a file outside the storage dir
        let outside = tmp.path().parent().unwrap().join("outside.txt");
        std::fs::write(&outside, "secret").unwrap();

        let result = storage.delete("../../outside.txt").await;
        assert!(result.is_err(), "path traversal should be rejected");
        // File should still exist
        assert!(outside.exists());
        std::fs::remove_file(outside).ok();
    }

    #[tokio::test]
    async fn delete_nonexistent_file_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(tmp.path().to_str().unwrap());
        let result = storage.delete("nonexistent/file.jpg").await;
        assert!(result.is_err());
    }

    // ── init tests ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn init_creates_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("nested/storage");
        let storage = LocalStorage::new(sub.to_str().unwrap());
        storage.init().await.unwrap();
        assert!(sub.exists());
        assert!(sub.is_dir());
    }

    // ── factory tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn factory_s3_without_feature_returns_error() {
        #[cfg(not(feature = "s3"))]
        {
            let tmp = tempfile::tempdir().unwrap();
            let mut config = test_config(tmp.path().to_str().unwrap());
            config.storage_backend = "s3".into();
            let result = create_storage(&config).await;
            match result {
                Err(msg) => assert!(msg.contains("feature flag"), "unexpected error: {msg}"),
                Ok(_) => panic!("expected error for s3 without feature"),
            }
        }
    }

    #[tokio::test]
    async fn factory_local_creates_storage() {
        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(tmp.path().to_str().unwrap());
        let storage = create_storage(&config).await.unwrap();
        let key = storage.upload(Uuid::new_v4(), "test.jpg", b"data").await.unwrap();
        assert!(key.ends_with(".jpg"));
    }

    fn test_config(storage_path: &str) -> AppConfig {
        AppConfig {
            database_url: String::new(),
            jwt_secret: "x".repeat(32),
            jwt_access_ttl_secs: 900,
            jwt_refresh_ttl_days: 30,
            listen_addr: "0.0.0.0:8080".into(),
            barcode_prefix: "HOM".into(),
            barcode_pad_width: 6,
            storage_path: storage_path.into(),
            downloads_path: "./downloads".into(),
            max_batch_size: 500,
            cors_origins: vec!["*".into()],
            db_max_connections: 20,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            db_max_lifetime_secs: 1800,
            max_upload_bytes: 10_485_760,
            allowed_image_mimes: vec!["image/jpeg".into()],
            rate_limit_enabled: false,
            rate_limit_rps: 50,
            rate_limit_burst: 200,
            storage_backend: "local".into(),
            s3_bucket: String::new(),
            s3_region: "us-east-1".into(),
            s3_endpoint: None,
            s3_prefix: "images".into(),
            request_timeout_secs: 30,
            upload_timeout_secs: 120,
            log_format: "text".into(),
        }
    }
}
