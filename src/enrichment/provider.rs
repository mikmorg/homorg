use crate::models::enrichment::{EnrichmentError, EnrichmentInput, EnrichmentOutput};

/// A pluggable backend for enriching an item's metadata from its images and
/// external codes.
///
/// Implementations must be `Send + Sync` so the daemon can wrap them in
/// `Box<dyn EnrichmentProvider>` and share them across tokio worker tasks.
#[async_trait::async_trait]
pub trait EnrichmentProvider: Send + Sync {
    /// Short identifier used in logs and in `enrichment_tasks.provider`
    /// (e.g. `"claude_cli"`, `"claude_api"`).
    fn name(&self) -> &'static str;

    /// Provider + model identifier stored in `EventMetadata.ai_model` so the
    /// item-detail UI can show which model authored which update
    /// (e.g. `"claude_cli:claude-opus-4-6"`).
    fn model_version(&self) -> String;

    /// Run a single enrichment call. Implementations are expected to enforce
    /// their own timeout; callers add no wrapper timeout.
    async fn enrich(&self, input: EnrichmentInput) -> Result<EnrichmentOutput, EnrichmentError>;
}
