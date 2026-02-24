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

        fs::write(&file_path, data)
            .await
            .map_err(|e| AppError::Storage(format!("Failed to write file: {e}")))?;

        let key = format!("{}/{}", item_id, storage_filename);
        Ok(key)
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        let file_path = self.base_path.join(key);
        if file_path.exists() {
            fs::remove_file(&file_path)
                .await
                .map_err(|e| AppError::Storage(format!("Failed to delete file: {e}")))?;
        }
        Ok(())
    }

    fn get_url(&self, key: &str) -> String {
        format!("/files/{key}")
    }
}
