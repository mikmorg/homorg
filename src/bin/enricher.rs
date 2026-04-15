//! Enrichment daemon: drains `enrichment_tasks` by running a provider and
//! writing the result back as a normal `ItemUpdated` event.
//!
//! Lifecycle per cycle:
//!   1. Sleep `ENRICHMENT_POLL_INTERVAL_SECS`.
//!   2. `claim_next_task` (SELECT ... FOR UPDATE SKIP LOCKED) — returns None
//!      if the queue is empty and we fall back to sleep.
//!   3. Build an [`EnrichmentInput`] from the item's current state, its
//!      images (staged to a scratch dir), and the taxonomy tables.
//!   4. Call the provider. On error, `fail_task` decides `pending` vs `dead`
//!      based on retryability + `max_attempts`.
//!   5. Auto-apply if the item has no human edits AND confidence ≥ threshold;
//!      otherwise stash the result in `ai_suggestions` for admin review.
//!   6. `complete_task` marks the row succeeded.
//!
//! Graceful shutdown on SIGINT / SIGTERM mirrors the API server.
//!
//! The daemon intentionally does NOT share `AppState` with the API — it only
//! needs pool + storage + event_store + provider, and keeping its
//! bootstrap standalone means a daemon bug can't affect API request paths.

use std::error::Error;
use std::process;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

type BoxError = Box<dyn Error + Send + Sync>;

use homorg::config::AppConfig;
use homorg::db;
use homorg::enrichment::{
    apply_as_item_updated, build_enrichment_input, stash_as_suggestion, ClaudeCliConfig, ClaudeCliProvider,
    CurrentFields, EnrichmentProvider,
};
use homorg::events::store::EventStore;
use homorg::models::enrichment::{EnrichmentError, EnrichmentStatus, EnrichmentTask, EnrichmentTrigger};
use homorg::queries::enrichment_queries::{claim_next_task, complete_task, fail_task};
use homorg::storage::{create_storage, StorageBackend};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        error!(error = ?e, "enricher exited with error");
        process::exit(1);
    }
}

async fn run() -> Result<(), BoxError> {
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env().map_err(|e| format!("config: {e}"))?;

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("enricher=debug,homorg=info,info"));
    if config.log_format == "json" {
        tracing_subscriber::fmt().json().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    if !config.enrichment_enabled {
        warn!("ENRICHMENT_ENABLED is false — exiting. Set ENRICHMENT_ENABLED=true to start the daemon.");
        return Ok(());
    }

    info!(
        poll_interval_secs = config.enrichment_poll_interval_secs,
        threshold = config.enrichment_auto_apply_threshold,
        model = %config.claude_cli_model,
        "enricher starting"
    );

    let pool = db::create_pool(&config)
        .await
        .map_err(|e| format!("create pool: {e}"))?;
    let storage = create_storage(&config).await.map_err(|e| format!("storage: {e}"))?;

    // Event notify channel: the daemon has no SSE subscribers of its own,
    // but publishing still wakes API-side subscribers that *do*. A capacity
    // of 8 is enough — a tight burst of enrichments just coalesces.
    let (event_notify, _) = tokio::sync::broadcast::channel(8);
    let event_store = EventStore::new(pool.clone(), event_notify);

    let provider = ClaudeCliProvider::new(ClaudeCliConfig {
        cli_path: std::path::PathBuf::from(&config.claude_cli_path),
        model: config.claude_cli_model.clone(),
        per_call_budget_usd: config.claude_cli_budget_usd,
        timeout: Duration::from_secs(config.claude_cli_timeout_secs),
        neutral_cwd: std::path::PathBuf::from("/tmp"),
    });

    let claimed_by = format!("{}:{}", hostname_or_unknown(), process::id());
    info!(%claimed_by, "enricher ready");

    let mut shutdown = Box::pin(shutdown_signal());

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("shutdown signal received, exiting");
                return Ok(());
            }
            _ = tokio::time::sleep(Duration::from_secs(config.enrichment_poll_interval_secs)) => {}
        }

        match process_one(&pool, &storage, &event_store, &provider, &config, &claimed_by).await {
            Ok(Some(task_id)) => info!(%task_id, "task processed"),
            Ok(None) => {} // queue empty, loop and wait
            Err(e) => warn!(error = %e, "enrichment cycle failed"),
        }
    }
}

/// Claim and process a single task. Returns the task_id on success, None if
/// the queue was empty. Errors from here are internal (fetch failed, etc.) —
/// provider errors are translated into `fail_task` calls and return `Ok`.
async fn process_one(
    pool: &PgPool,
    storage: &Arc<dyn StorageBackend>,
    event_store: &EventStore,
    provider: &ClaudeCliProvider,
    config: &AppConfig,
    claimed_by: &str,
) -> Result<Option<Uuid>, BoxError> {
    let Some(task) = claim_next_task(pool, claimed_by).await? else {
        return Ok(None);
    };
    let task_id = task.id;
    let item_id = task.item_id;
    info!(%task_id, %item_id, trigger = %task.trigger_event, "claimed task");

    if let Err(e) = dispatch_task(pool, storage, event_store, provider, config, &task).await {
        warn!(%task_id, error = %e, "dispatch failed");
        fail_task(pool, task_id, &e).await?;
        return Ok(Some(task_id));
    }

    complete_task(
        pool,
        task_id,
        provider.name(),
        &serde_json::json!({
            "model": provider.model_version(),
        }),
    )
    .await?;
    Ok(Some(task_id))
}

/// Build input, call provider, and route the output. Returns
/// `Err(EnrichmentError)` on any failure so the caller can translate via
/// `fail_task`.
async fn dispatch_task(
    pool: &PgPool,
    storage: &Arc<dyn StorageBackend>,
    event_store: &EventStore,
    provider: &ClaudeCliProvider,
    config: &AppConfig,
    task: &EnrichmentTask,
) -> Result<(), EnrichmentError> {
    // Sanity: don't attempt to process tasks that are already terminal (should
    // not happen with `claim_next_task` but cheap to guard).
    if !matches!(task.status, EnrichmentStatus::InProgress) {
        return Err(EnrichmentError::Other(format!(
            "claimed task {} in unexpected status {:?}",
            task.id, task.status
        )));
    }

    let trigger = parse_trigger(&task.trigger_event);

    // Scratch dir auto-deletes when this function returns (success or failure).
    let scratch = tempfile::tempdir().map_err(EnrichmentError::Io)?;
    let input = build_enrichment_input(pool, storage, scratch.path(), task.item_id, task.id, trigger).await?;

    // Snapshot pre-provider state so the dispatch layer can compute diffs
    // without having to re-query. The field names mirror `CurrentFields`.
    let current = CurrentFields {
        name: input.existing_name.clone(),
        description: input.existing_description.clone(),
        category: input.existing_category.clone(),
        tags: input.existing_tags.clone(),
        metadata: input.existing_metadata.clone(),
        // classification_confidence / needs_review aren't carried on the input
        // (the provider doesn't need them), so fetch them cheaply here.
        classification_confidence: fetch_confidence(pool, task.item_id).await?,
        needs_review: fetch_needs_review(pool, task.item_id).await?,
    };
    let user_edited = input.user_edited;

    let output = provider.enrich(input).await?;
    let auto_apply = !user_edited && output.confidence >= config.enrichment_auto_apply_threshold;
    info!(
        task_id = %task.id,
        confidence = output.confidence,
        user_edited,
        auto_apply,
        "provider returned"
    );

    if auto_apply {
        apply_as_item_updated(pool, event_store, task.item_id, task.id, &current, &output, provider)
            .await
            .map_err(|e| EnrichmentError::Other(format!("apply: {e}")))?;
    } else {
        stash_as_suggestion(
            pool,
            event_store,
            task.item_id,
            task.id,
            &current,
            &output,
            provider,
            chrono::Utc::now(),
        )
        .await
        .map_err(|e| EnrichmentError::Other(format!("stash: {e}")))?;
    }
    Ok(())
}

fn parse_trigger(s: &str) -> EnrichmentTrigger {
    match s {
        "external_code_added" => EnrichmentTrigger::ExternalCodeAdded,
        "manual_rerun" => EnrichmentTrigger::ManualRerun,
        "follow_up" => EnrichmentTrigger::FollowUp,
        _ => EnrichmentTrigger::ImageAdded,
    }
}

async fn fetch_confidence(pool: &PgPool, item_id: Uuid) -> Result<Option<f32>, EnrichmentError> {
    sqlx::query_scalar::<_, Option<f32>>("SELECT classification_confidence FROM items WHERE id = $1")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .map_err(|e| EnrichmentError::Other(format!("fetch confidence: {e}")))
}

async fn fetch_needs_review(pool: &PgPool, item_id: Uuid) -> Result<bool, EnrichmentError> {
    sqlx::query_scalar::<_, bool>("SELECT needs_review FROM items WHERE id = $1")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .map_err(|e| EnrichmentError::Other(format!("fetch needs_review: {e}")))
}

fn hostname_or_unknown() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("received SIGINT"),
        _ = terminate => info!("received SIGTERM"),
    }
}
