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
