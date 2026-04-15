//! Turns an [`EnrichmentOutput`] into an `ItemUpdated` event and routes it
//! through the event store.
//!
//! Two paths:
//!
//! - [`apply_as_item_updated`] — writes the proposed fields directly. Used
//!   when the provider is confident and no human edit exists on the item.
//! - [`stash_as_suggestion`] — leaves human-authored fields alone and only
//!   sets `ai_suggestions` / `needs_review` / `classification_confidence`,
//!   so the admin review UI can show a diff and apply field-by-field.
//!
//! Both paths produce **a single** `ItemUpdated` event authored by the
//! `ai-enricher` system user, with `EventMetadata.ai_model` and
//! `ai_task_id` set. Because the event type is reused, existing undo and
//! history projections work without changes.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::enrichment::provider::EnrichmentProvider;
use crate::errors::{AppError, AppResult};
use crate::events::projector::Projector;
use crate::events::store::EventStore;
use crate::models::enrichment::{AiSuggestions, EnrichmentOutput, AI_ENRICHER_USER_ID};
use crate::models::event::{DomainEvent, EventMetadata, FieldChange, ItemUpdatedData};

/// Snapshot of the item's current user-visible fields before the provider
/// ran. Used to compute field-level diffs and to merge additive updates
/// (metadata, tags).
pub struct CurrentFields {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub metadata: Value,
    pub classification_confidence: Option<f32>,
    pub needs_review: bool,
}

/// Auto-apply path: write the provider's proposed fields directly. Runs
/// only when the provider's confidence is above the threshold and the item
/// has no human edits.
#[allow(clippy::too_many_arguments)]
pub async fn apply_as_item_updated(
    pool: &PgPool,
    event_store: &EventStore,
    item_id: Uuid,
    task_id: Uuid,
    current: &CurrentFields,
    output: &EnrichmentOutput,
    provider: &dyn EnrichmentProvider,
) -> AppResult<()> {
    let mut changes = diff_fields(current, output);

    // Clear any prior suggestion blob — if we're auto-applying, the review
    // queue shouldn't still surface this item. needs_review flips off for
    // the same reason.
    changes.push(FieldChange {
        field: "ai_suggestions".into(),
        old: Value::Null,
        new: Value::Null,
    });
    changes.push(FieldChange {
        field: "needs_review".into(),
        old: Value::Bool(current.needs_review),
        new: Value::Bool(false),
    });
    changes.push(FieldChange {
        field: "classification_confidence".into(),
        old: current
            .classification_confidence
            .map(|f| Value::from(f as f64))
            .unwrap_or(Value::Null),
        new: Value::from(output.confidence as f64),
    });

    // No actual diffs and only confidence bookkeeping? Still append so the
    // event log records the enrichment attempt with its reasoning — this is
    // what lets admins distinguish "AI was run but agreed with existing
    // values" from "AI was never run on this item".
    append_item_updated(pool, event_store, item_id, task_id, changes, provider).await
}

/// Stash path: leave user-authored fields alone. Only touch the three
/// review-related columns + classification_confidence.
#[allow(clippy::too_many_arguments)]
pub async fn stash_as_suggestion(
    pool: &PgPool,
    event_store: &EventStore,
    item_id: Uuid,
    task_id: Uuid,
    current: &CurrentFields,
    output: &EnrichmentOutput,
    provider: &dyn EnrichmentProvider,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let suggestions = AiSuggestions {
        task_id,
        generated_at: now,
        model: provider.model_version(),
        confidence: output.confidence,
        name: output.name.clone(),
        description: output.description.clone(),
        tags: output.tags.clone(),
        category: output.category.clone(),
        metadata_additions: output.metadata_additions.clone(),
        discovered_codes: output.discovered_codes.clone(),
        reasoning: output.reasoning.clone(),
    };
    let suggestions_json =
        serde_json::to_value(&suggestions).map_err(|e| AppError::Internal(format!("serialize AiSuggestions: {e}")))?;

    let changes = vec![
        FieldChange {
            field: "ai_suggestions".into(),
            old: Value::Null,
            new: suggestions_json,
        },
        FieldChange {
            field: "needs_review".into(),
            old: Value::Bool(current.needs_review),
            new: Value::Bool(true),
        },
        FieldChange {
            field: "classification_confidence".into(),
            old: current
                .classification_confidence
                .map(|f| Value::from(f as f64))
                .unwrap_or(Value::Null),
            new: Value::from(output.confidence as f64),
        },
    ];

    append_item_updated(pool, event_store, item_id, task_id, changes, provider).await
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn diff_fields(current: &CurrentFields, output: &EnrichmentOutput) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    if let Some(name) = &output.name {
        if current.name.as_deref() != Some(name.as_str()) {
            changes.push(FieldChange {
                field: "name".into(),
                old: current.name.clone().map(Value::String).unwrap_or(Value::Null),
                new: Value::String(name.clone()),
            });
        }
    }
    if let Some(desc) = &output.description {
        if current.description.as_deref() != Some(desc.as_str()) {
            changes.push(FieldChange {
                field: "description".into(),
                old: current.description.clone().map(Value::String).unwrap_or(Value::Null),
                new: Value::String(desc.clone()),
            });
        }
    }
    if let Some(cat) = &output.category {
        if current.category.as_deref() != Some(cat.as_str()) {
            changes.push(FieldChange {
                field: "category".into(),
                old: current.category.clone().map(Value::String).unwrap_or(Value::Null),
                new: Value::String(cat.clone()),
            });
        }
    }

    // Tags are additive — union existing with proposed. The projector's tags
    // branch expects the *full* target set (it DELETEs and re-INSERTs), so
    // we pass the union, not just the new tags.
    if !output.tags.is_empty() {
        let mut merged: Vec<String> = current.tags.clone();
        for t in &output.tags {
            if !merged.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                merged.push(t.clone());
            }
        }
        if merged.len() != current.tags.len() {
            changes.push(FieldChange {
                field: "tags".into(),
                old: json!(current.tags),
                new: json!(merged),
            });
        }
    }

    // Metadata additions are deep-merged into existing metadata so the AI
    // can't wipe user-authored metadata keys.
    if !output.metadata_additions.is_null() {
        let mut merged = current.metadata.clone();
        deep_merge(&mut merged, &output.metadata_additions);
        if merged != current.metadata {
            changes.push(FieldChange {
                field: "metadata".into(),
                old: current.metadata.clone(),
                new: merged,
            });
        }
    }

    changes
}

fn deep_merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (k, v) in p {
                match t.get_mut(k) {
                    Some(existing) => deep_merge(existing, v),
                    None => {
                        t.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (t, p) => {
            // Scalars and mismatched types: patch wins.
            *t = p.clone();
        }
    }
}

async fn append_item_updated(
    pool: &PgPool,
    event_store: &EventStore,
    item_id: Uuid,
    task_id: Uuid,
    changes: Vec<FieldChange>,
    provider: &dyn EnrichmentProvider,
) -> AppResult<()> {
    let event = DomainEvent::ItemUpdated(ItemUpdatedData { changes });
    let metadata = EventMetadata {
        ai_model: Some(provider.model_version()),
        ai_task_id: Some(task_id),
        ..Default::default()
    };

    let mut tx = pool.begin().await?;
    event_store
        .append_in_tx(&mut tx, item_id, &event, AI_ENRICHER_USER_ID, &metadata)
        .await?;
    Projector::apply(&mut tx, item_id, &event, AI_ENRICHER_USER_ID).await?;
    event_store.commit_and_notify(tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_current() -> CurrentFields {
        CurrentFields {
            name: None,
            description: None,
            category: None,
            tags: vec![],
            metadata: json!({}),
            classification_confidence: None,
            needs_review: false,
        }
    }

    #[test]
    fn diff_fields_emits_only_changed_fields() {
        let current = CurrentFields {
            name: Some("Old".into()),
            description: Some("Same".into()),
            category: None,
            tags: vec!["a".into()],
            metadata: json!({"keep": 1}),
            ..empty_current()
        };
        let out = EnrichmentOutput {
            name: Some("New".into()),
            description: Some("Same".into()),
            category: Some("Books".into()),
            tags: vec!["a".into(), "b".into()],
            metadata_additions: json!({"new_key": 2}),
            confidence: 0.9,
            ..Default::default()
        };
        let changes = diff_fields(&current, &out);
        let fields: Vec<&str> = changes.iter().map(|c| c.field.as_str()).collect();
        assert!(fields.contains(&"name"));
        assert!(!fields.contains(&"description"), "same value shouldn't diff");
        assert!(fields.contains(&"category"));
        assert!(fields.contains(&"tags"));
        assert!(fields.contains(&"metadata"));
    }

    #[test]
    fn diff_tags_are_unioned_case_insensitive() {
        let current = CurrentFields {
            tags: vec!["Book".into(), "fiction".into()],
            ..empty_current()
        };
        let out = EnrichmentOutput {
            tags: vec!["book".into(), "classic".into()],
            confidence: 0.9,
            ..Default::default()
        };
        let changes = diff_fields(&current, &out);
        let tags_change = changes.iter().find(|c| c.field == "tags").expect("tags change");
        let merged: Vec<String> = serde_json::from_value(tags_change.new.clone()).unwrap();
        assert_eq!(merged, vec!["Book", "fiction", "classic"]);
    }

    #[test]
    fn deep_merge_preserves_unmentioned_keys() {
        let mut a = json!({"book": {"author": "X", "year": 1999}});
        let b = json!({"book": {"year": 2000, "pages": 300}});
        deep_merge(&mut a, &b);
        assert_eq!(a["book"]["author"], "X");
        assert_eq!(a["book"]["year"], 2000);
        assert_eq!(a["book"]["pages"], 300);
    }

    #[test]
    fn metadata_additions_dont_clobber_existing() {
        let current = CurrentFields {
            metadata: json!({"user_note": "mine", "book": {"user_rating": 5}}),
            ..empty_current()
        };
        let out = EnrichmentOutput {
            metadata_additions: json!({"book": {"author": "AI", "year": 2024}}),
            confidence: 0.9,
            ..Default::default()
        };
        let changes = diff_fields(&current, &out);
        let meta = changes.iter().find(|c| c.field == "metadata").expect("metadata");
        assert_eq!(meta.new["user_note"], "mine");
        assert_eq!(meta.new["book"]["user_rating"], 5);
        assert_eq!(meta.new["book"]["author"], "AI");
        assert_eq!(meta.new["book"]["year"], 2024);
    }
}
