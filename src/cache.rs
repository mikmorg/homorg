//! In-process cache layer using moka for hot query paths.
//!
//! Caches taxonomy data (tags, categories, container types) and barcode
//! resolutions with TTL-based invalidation. All caches are bounded by
//! entry count to prevent unbounded memory growth.

use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

use crate::models::barcode::BarcodeResolution;

/// Shared cache instance holding all application caches.
#[derive(Clone)]
pub struct AppCache {
    /// Barcode → resolution cache. High hit rate during scanning sessions.
    pub barcode: Cache<String, Arc<BarcodeResolution>>,
    /// Tags list cache (all tags, rarely changes).
    pub tags: Cache<String, Arc<serde_json::Value>>,
    /// Categories list cache.
    pub categories: Cache<String, Arc<serde_json::Value>>,
    /// Container types list cache.
    pub container_types: Cache<String, Arc<serde_json::Value>>,
}

impl AppCache {
    pub fn new() -> Self {
        Self {
            barcode: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(300)) // 5 min TTL
                .build(),
            tags: Cache::builder()
                .max_capacity(10)
                .time_to_live(Duration::from_secs(60)) // 1 min TTL
                .build(),
            categories: Cache::builder()
                .max_capacity(10)
                .time_to_live(Duration::from_secs(60))
                .build(),
            container_types: Cache::builder()
                .max_capacity(10)
                .time_to_live(Duration::from_secs(60))
                .build(),
        }
    }

    /// Invalidate taxonomy caches (call after tag/category/container-type mutation).
    pub fn invalidate_taxonomy(&self) {
        // invalidate_all is synchronous on moka future::Cache — it schedules
        // eviction but does not return a future.
        self.tags.invalidate_all();
        self.categories.invalidate_all();
        self.container_types.invalidate_all();
    }

    /// Invalidate a specific barcode entry (call after barcode assignment/change).
    pub async fn invalidate_barcode(&self, barcode: &str) {
        self.barcode.invalidate(barcode).await;
    }
}

impl Default for AppCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_insert_and_retrieve() {
        let cache = AppCache::new();
        let value = Arc::new(serde_json::json!(["tag1", "tag2"]));
        cache.tags.insert("all".to_string(), value.clone()).await;

        let retrieved = cache.tags.get("all").await;
        assert!(retrieved.is_some());
        assert_eq!(*retrieved.unwrap(), *value);
    }

    #[tokio::test]
    async fn cache_miss_returns_none() {
        let cache = AppCache::new();
        assert!(cache.tags.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn invalidate_taxonomy_clears_all_taxonomy_caches() {
        let cache = AppCache::new();
        cache
            .tags
            .insert("all".to_string(), Arc::new(serde_json::json!([])))
            .await;
        cache
            .categories
            .insert("all".to_string(), Arc::new(serde_json::json!([])))
            .await;
        cache
            .container_types
            .insert("all".to_string(), Arc::new(serde_json::json!([])))
            .await;

        cache.invalidate_taxonomy();
        // After invalidate_all, pending entries are scheduled for removal.
        // Run pending tasks to process eviction.
        cache.tags.run_pending_tasks().await;
        cache.categories.run_pending_tasks().await;
        cache.container_types.run_pending_tasks().await;

        assert!(cache.tags.get("all").await.is_none());
        assert!(cache.categories.get("all").await.is_none());
        assert!(cache.container_types.get("all").await.is_none());
    }

    #[tokio::test]
    async fn invalidate_barcode_removes_entry() {
        let cache = AppCache::new();
        let resolution = BarcodeResolution::System {
            barcode: "HOM-000001".into(),
            item_id: uuid::Uuid::new_v4(),
        };
        cache
            .barcode
            .insert("HOM-000001".to_string(), Arc::new(resolution))
            .await;
        assert!(cache.barcode.get("HOM-000001").await.is_some());

        cache.invalidate_barcode("HOM-000001").await;
        cache.barcode.run_pending_tasks().await;
        assert!(cache.barcode.get("HOM-000001").await.is_none());
    }

    #[tokio::test]
    async fn barcode_cache_does_not_affect_taxonomy() {
        let cache = AppCache::new();
        cache
            .tags
            .insert("all".to_string(), Arc::new(serde_json::json!(["t1"])))
            .await;
        let resolution = BarcodeResolution::System {
            barcode: "HOM-000001".into(),
            item_id: uuid::Uuid::new_v4(),
        };
        cache
            .barcode
            .insert("HOM-000001".to_string(), Arc::new(resolution))
            .await;

        cache.invalidate_barcode("HOM-000001").await;
        cache.barcode.run_pending_tasks().await;

        // Tags should be unaffected
        assert!(cache.tags.get("all").await.is_some());
    }

    #[test]
    fn default_creates_cache() {
        let cache = AppCache::default();
        // Just verify it constructs without panic
        assert_eq!(cache.barcode.entry_count(), 0);
    }
}
