//! AI enrichment pipeline: pluggable model providers + shared input/output
//! types (re-exported from [`crate::models::enrichment`]).
//!
//! The daemon crate `src/bin/enricher.rs` drives this module: claim a task,
//! build an [`EnrichmentInput`], call a provider via the [`EnrichmentProvider`]
//! trait, and write the result back as an `ItemUpdated` event.

pub mod claude_cli;
pub mod provider;

pub use claude_cli::{ClaudeCliConfig, ClaudeCliProvider};
pub use provider::EnrichmentProvider;

// Re-export the wire types so `use crate::enrichment::EnrichmentInput;` works
// without reaching into `models::enrichment`.
pub use crate::models::enrichment::{
    EnrichmentError, EnrichmentImage, EnrichmentInput, EnrichmentOutput, PresetHint,
};
