use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

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
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        let storage_filename = format!("{file_id}.{ext}");
        let file_path = dir.join(&storage_filename);

        // RM-2: Atomic write — write to a temp file then rename so a crash during
        // write never leaves a partial/corrupt file at the final path.
        let tmp_path = file_path.with_extension(format!("{ext}.tmp"));
        fs::write(&tmp_path, data)
            .await
            .map_err(|e| AppError::Storage(format!("Failed to write temp file: {e}")))?;
        fs::rename(&tmp_path, &file_path)
            .await
            .map_err(|e| {
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
