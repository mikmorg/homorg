//! Data types for the AI enrichment pipeline (see migration 0025).
//!
//! The enricher daemon (`src/bin/enricher`) claims rows from `enrichment_tasks`,
//! invokes a pluggable [`EnrichmentProvider`](crate::enrichment::EnrichmentProvider),
//! and writes results back via `ItemUpdated` events authored by the
//! [`AI_ENRICHER_USER_ID`] system user.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::{uuid, Uuid};

/// Hardcoded UUID of the `ai-enricher` system user (see migration 0025).
///
/// Events authored by the daemon carry this value as their `actor_id` so the
/// audit trail clearly distinguishes AI updates from human updates.
pub const AI_ENRICHER_USER_ID: Uuid = uuid!("00000000-0000-0000-0000-00000000a1e1");

/// Default priority when an image upload triggers enrichment.
/// Lower = higher priority.
pub const PRIORITY_IMAGE_ADDED: i32 = 100;

/// Priority when an ISBN/UPC/EAN external code triggers enrichment —
/// runs ahead of image-only tasks because lookups are fast and authoritative.
pub const PRIORITY_EXTERNAL_CODE_ADDED: i32 = 50;

/// Priority for admin-requested re-runs; ahead of everything else.
pub const PRIORITY_MANUAL_RERUN: i32 = 25;

// ── Enums ────────────────────────────────────────────────────────────────────

/// Lifecycle of a single enrichment task row. Mirrors the Postgres
/// `enrichment_status` enum declared in migration 0025.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "enrichment_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentStatus {
    Pending,
    InProgress,
    Succeeded,
    Failed,
    Dead,
    Canceled,
}

/// Why a task was enqueued. Stored as a free-form VARCHAR in `trigger_event`
/// for flexibility — new triggers can be added without a schema change —
/// but the known values are modeled here for type safety on the write side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentTrigger {
    /// A new image was attached to the item (the common case).
    ImageAdded,
    /// A new external code (ISBN/UPC/EAN/…) was attached to the item.
    ExternalCodeAdded,
    /// An admin clicked "Re-run enrichment" on the item detail page.
    ManualRerun,
    /// A follow-up task queued by the daemon itself when events landed
    /// while the previous task was in flight.
    FollowUp,
}

impl EnrichmentTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImageAdded => "image_added",
            Self::ExternalCodeAdded => "external_code_added",
            Self::ManualRerun => "manual_rerun",
            Self::FollowUp => "follow_up",
        }
    }

    pub fn default_priority(self) -> i32 {
        match self {
            Self::ImageAdded => PRIORITY_IMAGE_ADDED,
            Self::ExternalCodeAdded => PRIORITY_EXTERNAL_CODE_ADDED,
            Self::ManualRerun => PRIORITY_MANUAL_RERUN,
            Self::FollowUp => PRIORITY_IMAGE_ADDED,
        }
    }
}

// ── DB row ───────────────────────────────────────────────────────────────────

/// A row from `enrichment_tasks`.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnrichmentTask {
    pub id: Uuid,
    pub item_id: Uuid,
    pub trigger_event: String,
    pub priority: i32,
    pub status: EnrichmentStatus,
    pub attempts: i32,
    pub max_attempts: i32,
    pub provider: Option<String>,
    pub last_error: Option<String>,
    pub result_summary: Option<serde_json::Value>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub claimed_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ── Provider I/O (used by `crate::enrichment` in Phase 2) ────────────────────

/// Image bytes plus the path the image lives at on disk. The provider will
/// usually need to expose the image to the model by path (CLI `--add-dir`)
/// or bytes (HTTP API inline block).
#[derive(Debug, Clone)]
pub struct EnrichmentImage {
    /// Stable path the caller can hand to the model (must be readable).
    /// For local storage this is the filesystem path; for S3 the caller
    /// downloads to a scratch dir first.
    pub local_path: std::path::PathBuf,
    /// Optional original URL (the `images[i].path` stored on the item),
    /// included for logging only.
    pub item_relative_path: String,
    /// Raw bytes in case the provider prefers inline.
    pub bytes: Vec<u8>,
}

/// Hint passed to the provider about what kind of item this is supposed to be,
/// derived from the preset barcode that created it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetHint {
    pub is_container: bool,
    /// Human-readable container type name if the item was created from a
    /// container preset (e.g. "Book", "Storage Bin", "USB Drive"). This is
    /// the strongest single signal of what the item actually is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_type_name: Option<String>,
}

/// Everything the daemon hands a provider to enrich a single item.
#[derive(Debug, Clone)]
pub struct EnrichmentInput {
    pub item_id: Uuid,
    pub task_id: Uuid,
    pub existing_name: Option<String>,
    pub existing_description: Option<String>,
    pub existing_tags: Vec<String>,
    pub existing_category: Option<String>,
    pub existing_metadata: serde_json::Value,
    pub external_codes: Vec<(String, String)>, // (type, value) e.g. ("ISBN", "978-...")
    pub preset_hint: Option<PresetHint>,
    pub images: Vec<EnrichmentImage>,
    pub available_categories: Vec<String>,
    pub available_tags: Vec<String>,
    pub allow_new_tags: bool,
    /// True if any non-AI `ItemUpdated` event exists for this item. Providers
    /// MAY use this as a signal to be less aggressive (e.g. produce lower
    /// confidence or more conservative suggestions) but daemon behaviour is
    /// already correct regardless — when `user_edited` is true the daemon
    /// stores suggestions instead of overwriting fields.
    pub user_edited: bool,
}

/// A provider's answer. `None` on optional fields means "I have nothing to
/// suggest for this field"; the daemon will leave the existing value alone.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnrichmentOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Extra data to deep-merge into `items.metadata` (e.g.
    /// `{"book": {"title": "...", "authors": ["..."], "publisher": "...", "year": 2024}}`).
    /// Already indexed by search_vector at weight D.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata_additions: serde_json::Value,
    /// ISBN / UPC / EAN codes the provider extracted from images or lookups.
    /// Daemon enforces the MAX_EXTERNAL_CODES cap when attaching.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discovered_codes: Vec<(String, String)>,
    /// 0.0 — 1.0. Below `ENRICHMENT_CONFIDENCE_THRESHOLD` flips needs_review.
    pub confidence: f32,
    /// Free-form explanation from the model; stored in `result_summary`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

/// Stored inside `items.metadata.ai_suggestions` when the daemon runs on an
/// already user-edited item. The review UI reads this blob to show a diff and
/// let the admin accept/reject the AI's suggestions field-by-field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSuggestions {
    pub task_id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub model: String,
    pub confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata_additions: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discovered_codes: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

// ── Errors ───────────────────────────────────────────────────────────────────

/// Provider-layer error. Retryability is decided by the daemon (e.g. BadOutput
/// retries, SpendCapped does not, Timeout retries with backoff).
#[derive(Debug, thiserror::Error)]
pub enum EnrichmentError {
    #[error("provider timed out after {0}s")]
    Timeout(u64),
    #[error("provider invocation failed: {0}")]
    Invocation(String),
    #[error("provider produced unparseable output: {0}")]
    BadOutput(String),
    #[error("spend cap exceeded: {0}")]
    SpendCapped(String),
    #[error("provider is not authenticated ({0}); set ANTHROPIC_API_KEY or run `claude /login`")]
    NotAuthenticated(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl EnrichmentError {
    /// Whether the daemon should retry this task on failure. SpendCapped and
    /// NotAuthenticated are operator-action errors; everything else retries.
    pub fn is_retryable(&self) -> bool {
        !matches!(self, Self::SpendCapped(_) | Self::NotAuthenticated(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_enricher_user_id_is_parseable() {
        // Sanity check that the uuid! macro matches the migration.
        assert_eq!(
            AI_ENRICHER_USER_ID.to_string(),
            "00000000-0000-0000-0000-00000000a1e1"
        );
    }

    #[test]
    fn trigger_to_str_and_priority() {
        assert_eq!(EnrichmentTrigger::ImageAdded.as_str(), "image_added");
        assert_eq!(
            EnrichmentTrigger::ExternalCodeAdded.as_str(),
            "external_code_added"
        );
        assert_eq!(EnrichmentTrigger::ManualRerun.as_str(), "manual_rerun");
        assert_eq!(EnrichmentTrigger::FollowUp.as_str(), "follow_up");

        assert!(
            EnrichmentTrigger::ManualRerun.default_priority()
                < EnrichmentTrigger::ExternalCodeAdded.default_priority()
        );
        assert!(
            EnrichmentTrigger::ExternalCodeAdded.default_priority()
                < EnrichmentTrigger::ImageAdded.default_priority()
        );
    }

    #[test]
    fn status_serde_is_snake_case() {
        let json = serde_json::to_string(&EnrichmentStatus::InProgress).unwrap();
        assert_eq!(json, "\"in_progress\"");
        let back: EnrichmentStatus = serde_json::from_str("\"succeeded\"").unwrap();
        assert_eq!(back, EnrichmentStatus::Succeeded);
    }

    #[test]
    fn output_serde_roundtrip() {
        let out = EnrichmentOutput {
            name: Some("The Pragmatic Programmer".into()),
            description: Some("Classic book on software craft.".into()),
            tags: vec!["programming".into(), "classic".into()],
            category: Some("Books".into()),
            metadata_additions: serde_json::json!({
                "book": { "authors": ["Hunt", "Thomas"], "year": 1999 }
            }),
            discovered_codes: vec![("ISBN".into(), "978-0-201-61622-4".into())],
            confidence: 0.92,
            reasoning: Some("Matched ISBN to OpenLibrary record.".into()),
        };
        let s = serde_json::to_string(&out).unwrap();
        let back: EnrichmentOutput = serde_json::from_str(&s).unwrap();
        assert_eq!(back.name, out.name);
        assert_eq!(back.tags, out.tags);
        assert_eq!(back.discovered_codes, out.discovered_codes);
        assert!((back.confidence - out.confidence).abs() < 1e-6);
    }

    #[test]
    fn ai_suggestions_serde_roundtrip() {
        let s = AiSuggestions {
            task_id: Uuid::nil(),
            generated_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            model: "claude_cli:claude-opus-4-6".into(),
            confidence: 0.81,
            name: Some("Suggested".into()),
            description: None,
            tags: vec![],
            category: None,
            metadata_additions: serde_json::Value::Null,
            discovered_codes: vec![],
            reasoning: Some("why".into()),
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["model"], "claude_cli:claude-opus-4-6");
        let back: AiSuggestions = serde_json::from_value(json).unwrap();
        assert_eq!(back.name.as_deref(), Some("Suggested"));
        assert_eq!(back.confidence, 0.81);
    }

    #[test]
    fn error_retryability() {
        assert!(EnrichmentError::Timeout(90).is_retryable());
        assert!(EnrichmentError::BadOutput("nope".into()).is_retryable());
        assert!(!EnrichmentError::SpendCapped("$0.50 exceeded".into()).is_retryable());
        assert!(!EnrichmentError::NotAuthenticated("no key".into()).is_retryable());
    }
}
