//! Gathers everything an [`EnrichmentProvider`] needs for a single task.
//!
//! Separating this from the daemon's main loop keeps the dispatch logic in
//! `src/bin/enricher.rs` short and testable, and makes the "what does an
//! enrichment see?" question answerable in one file.
//!
//! The builder is responsible for three tricky pieces:
//!
//! 1. **`user_edited` detection.** A non-AI `ItemUpdated` event in the
//!    history means a human has touched the item, so auto-apply is disabled
//!    even for a high-confidence result — we route the result through the
//!    review queue instead.
//!
//! 2. **Image staging.** Paths in `items.images` are storage-backend keys
//!    (either local FS relative paths or S3 keys). The provider needs the
//!    bytes on disk in a deterministic layout so it can feed them to the
//!    model's Read tool. We download everything into the caller-owned
//!    `scratch_dir` with filenames `img_0.<ext>`, `img_1.<ext>`, ….
//!
//! 3. **Taxonomy constraints.** The prompt includes `available_tags` and
//!    `available_categories` so the model is constrained to the existing
//!    taxonomy. `allow_new_tags = true` loosens this for free-form triggers
//!    (image-added, manual-rerun); external-code-added keeps it `false`.

use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::models::enrichment::{
    EnrichmentError, EnrichmentImage, EnrichmentInput, EnrichmentTrigger, PresetHint, AI_ENRICHER_USER_ID,
};
use crate::storage::StorageBackend;

/// Max images passed to the provider per call. Newest-first; very old photos
/// usually add noise without extra signal and cost tokens.
const MAX_IMAGES_PER_CALL: usize = 5;

/// Build the full [`EnrichmentInput`] for a task.
///
/// `scratch_dir` is caller-owned (typically a [`tempfile::TempDir`]) — this
/// function writes image files into it but does not delete them. The caller
/// drops the dir after the provider call returns.
pub async fn build_enrichment_input(
    pool: &PgPool,
    storage: &Arc<dyn StorageBackend>,
    scratch_dir: &Path,
    item_id: Uuid,
    task_id: Uuid,
    trigger: EnrichmentTrigger,
) -> Result<EnrichmentInput, EnrichmentError> {
    let ctx = fetch_item_context(pool, item_id).await?;
    let user_edited = has_non_ai_update(pool, item_id).await?;
    let preset_hint = fetch_preset_hint(pool, item_id).await?;
    let (available_categories, available_tags) = fetch_taxonomy(pool).await?;
    let images = stage_images(storage, scratch_dir, &ctx.images).await?;

    // ISBN lookups should produce exactly the authoritative title/subtitle
    // from the external source — discouraging new tags keeps those responses
    // disciplined. Image-driven triggers benefit from taxonomy expansion.
    let allow_new_tags = !matches!(trigger, EnrichmentTrigger::ExternalCodeAdded);

    Ok(EnrichmentInput {
        item_id,
        task_id,
        existing_name: ctx.name,
        existing_description: ctx.description,
        existing_tags: ctx.tags,
        existing_category: ctx.category,
        existing_metadata: ctx.metadata,
        external_codes: ctx.external_codes,
        preset_hint,
        images,
        available_categories,
        available_tags,
        allow_new_tags,
        user_edited,
    })
}

// ── Helpers ──────────────────────────────────────────────────────────────────

struct ItemContext {
    name: Option<String>,
    description: Option<String>,
    tags: Vec<String>,
    category: Option<String>,
    metadata: serde_json::Value,
    external_codes: Vec<(String, String)>,
    images: Vec<StoredImage>,
}

#[derive(Debug, Deserialize)]
struct StoredImage {
    path: String,
    #[serde(default)]
    #[allow(dead_code)]
    caption: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    order: i32,
}

#[derive(Debug, Deserialize)]
struct StoredCode {
    #[serde(rename = "type")]
    code_type: String,
    value: String,
}

async fn fetch_item_context(pool: &PgPool, item_id: Uuid) -> Result<ItemContext, EnrichmentError> {
    // One query pulling exactly what the input builder needs — avoids loading
    // the full Item/joins just to throw most of it away.
    let row: (
        Option<String>,
        Option<String>,
        Option<String>,
        serde_json::Value,
        serde_json::Value,
        serde_json::Value,
    ) = sqlx::query_as(
        r#"
        SELECT
            i.name,
            i.description,
            cat.name AS category,
            i.metadata,
            i.external_codes,
            i.images
        FROM items i
        LEFT JOIN categories cat ON cat.id = i.category_id
        WHERE i.id = $1 AND i.is_deleted = FALSE
        "#,
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| EnrichmentError::Other(format!("fetch item: {e}")))?
    .ok_or_else(|| EnrichmentError::Other(format!("item {item_id} not found or deleted")))?;

    let tags: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT t.name
        FROM item_tags it
        JOIN tags t ON t.id = it.tag_id
        WHERE it.item_id = $1
        ORDER BY t.name
        "#,
    )
    .bind(item_id)
    .fetch_all(pool)
    .await
    .map_err(|e| EnrichmentError::Other(format!("fetch tags: {e}")))?;

    let external_codes: Vec<(String, String)> = parse_codes(&row.4);
    let images: Vec<StoredImage> = serde_json::from_value(row.5).unwrap_or_default();

    Ok(ItemContext {
        name: row.0,
        description: row.1,
        tags,
        category: row.2,
        metadata: row.3,
        external_codes,
        images,
    })
}

fn parse_codes(value: &serde_json::Value) -> Vec<(String, String)> {
    let Some(arr) = value.as_array() else { return vec![] };
    arr.iter()
        .filter_map(|v| serde_json::from_value::<StoredCode>(v.clone()).ok())
        .map(|c| (c.code_type, c.value))
        .collect()
}

/// True if any `ItemUpdated` event on this item was authored by a non-AI actor.
/// Presence of a human edit flips auto-apply off — the daemon stashes a
/// suggestion instead of overwriting the human's work.
async fn has_non_ai_update(pool: &PgPool, item_id: Uuid) -> Result<bool, EnrichmentError> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM event_store
            WHERE aggregate_id = $1
              AND event_type = 'ItemUpdated'
              AND (actor_id IS NULL OR actor_id <> $2)
        )
        "#,
    )
    .bind(item_id)
    .bind(AI_ENRICHER_USER_ID)
    .fetch_one(pool)
    .await
    .map_err(|e| EnrichmentError::Other(format!("user_edited check: {e}")))?;
    Ok(exists)
}

async fn fetch_preset_hint(pool: &PgPool, item_id: Uuid) -> Result<Option<PresetHint>, EnrichmentError> {
    let row: Option<(bool, Option<String>)> = sqlx::query_as(
        r#"
        SELECT i.is_container, ct.name
        FROM items i
        LEFT JOIN container_properties cp ON cp.item_id = i.id
        LEFT JOIN container_types ct ON ct.id = cp.container_type_id
        WHERE i.id = $1
        "#,
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| EnrichmentError::Other(format!("preset hint: {e}")))?;

    Ok(row.map(|(is_container, container_type_name)| PresetHint {
        is_container,
        container_type_name,
    }))
}

async fn fetch_taxonomy(pool: &PgPool) -> Result<(Vec<String>, Vec<String>), EnrichmentError> {
    let categories: Vec<String> = sqlx::query_scalar("SELECT name FROM categories ORDER BY name")
        .fetch_all(pool)
        .await
        .map_err(|e| EnrichmentError::Other(format!("categories: {e}")))?;
    let tags: Vec<String> = sqlx::query_scalar("SELECT name FROM tags ORDER BY name")
        .fetch_all(pool)
        .await
        .map_err(|e| EnrichmentError::Other(format!("tags: {e}")))?;
    Ok((categories, tags))
}

async fn stage_images(
    storage: &Arc<dyn StorageBackend>,
    scratch_dir: &Path,
    images: &[StoredImage],
) -> Result<Vec<EnrichmentImage>, EnrichmentError> {
    // Newest-first ordering already applied at the caller level? The images
    // JSONB has no explicit timestamp, so we preserve insertion order then
    // take the trailing `MAX_IMAGES_PER_CALL` (most recently added).
    let take_from = images.len().saturating_sub(MAX_IMAGES_PER_CALL);
    let mut staged = Vec::with_capacity(MAX_IMAGES_PER_CALL);
    for (i, img) in images.iter().enumerate().skip(take_from) {
        let bytes = match storage.download(&img.path).await {
            Ok(b) => b,
            Err(e) => {
                // Missing or unreadable image: skip rather than fail the whole
                // task. The model can still work from other images + codes.
                tracing::warn!(path = %img.path, error = %e, "stage_images: download failed, skipping");
                continue;
            }
        };
        let ext = extension_for(&img.path);
        let filename = format!("img_{}.{ext}", i - take_from);
        let local_path = scratch_dir.join(&filename);
        let mut f = tokio::fs::File::create(&local_path)
            .await
            .map_err(EnrichmentError::Io)?;
        f.write_all(&bytes).await.map_err(EnrichmentError::Io)?;
        f.flush().await.map_err(EnrichmentError::Io)?;
        staged.push(EnrichmentImage {
            local_path,
            item_relative_path: img.path.clone(),
            bytes,
        });
    }
    Ok(staged)
}

fn extension_for(path: &str) -> &str {
    let dot = path.rfind('.').map(|i| &path[i + 1..]).unwrap_or("jpg");
    match dot.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "jpg",
        "png" => "png",
        "webp" => "webp",
        "gif" => "gif",
        "avif" => "avif",
        "heic" => "heic",
        "heif" => "heif",
        _ => "jpg",
    }
}

// ── Marker for timestamps inside AiSuggestions when constructed by the daemon.
// Kept here because this file is the one place both the daemon's dispatch
// path and the admin approve path need the same clock source (UTC now).
#[inline]
pub fn utc_now() -> DateTime<Utc> {
    Utc::now()
}
