use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::{atomic::Ordering, Arc};

use crate::auth::middleware::AuthUser;
use crate::errors::{AppError, AppResult};
use crate::events::projector::Projector;
use crate::queries::stats_queries::StatsResponse;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/metrics", get(metrics))
        .route("/stats", get(stats))
        .route("/admin/rebuild-projections", post(rebuild_projections))
        .route("/admin/rebuild-status", get(rebuild_status))
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    database: String,
    /// OP-3: Include version so operators can confirm which build is running.
    version: &'static str,
    /// True when no users exist yet — the client should redirect to /setup.
    /// Only included when the database is reachable (avoids leaking state on errors).
    #[serde(skip_serializing_if = "Option::is_none")]
    setup_required: Option<bool>,
}

/// Liveness check with DB connectivity status.
/// Returns 200 when healthy, 503 when the database is unreachable.
async fn health(State(state): State<Arc<AppState>>) -> (StatusCode, Json<HealthResponse>) {
    let db_status = sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.pool).await;

    let (status_code, status, database) = match db_status {
        Ok(_) => (StatusCode::OK, "ok".to_string(), "connected".to_string()),
        Err(e) => {
            tracing::error!(error = %e, "Health check: database unreachable");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "degraded".to_string(),
                "unavailable".to_string(),
            )
        }
    };

    // H-5: Only check setup_required when DB is reachable.
    // Leaking this flag is acceptable for the initial setup UX flow;
    // the /auth/setup endpoint is locked after first use regardless.
    let setup_required = if status_code == StatusCode::OK {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE is_active = TRUE")
            .fetch_one(&state.pool)
            .await
            .unwrap_or(1);
        Some(count == 0)
    } else {
        None
    };

    (
        status_code,
        Json(HealthResponse {
            status,
            database,
            version: env!("CARGO_PKG_VERSION"),
            setup_required,
        }),
    )
}

/// Liveness probe — always returns 200 if the process is running.
/// Use for Kubernetes livenessProbe or basic load-balancer health.
async fn health_live() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "alive" })),
    )
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: String,
    database: DatabaseHealth,
    storage: String,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct DatabaseHealth {
    connected: bool,
    pool_size: u32,
    pool_idle: u32,
    pool_active: u32,
}

/// Readiness probe — returns 200 only when all dependencies are healthy.
/// Use for Kubernetes readinessProbe or load-balancer backend health.
async fn health_ready(State(state): State<Arc<AppState>>) -> (StatusCode, Json<ReadinessResponse>) {
    // DB connectivity check
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    let pool_size = state.pool.size();
    let pool_idle = state.pool.num_idle() as u32;
    let pool_active = pool_size.saturating_sub(pool_idle);

    // Storage check: verify base path exists and is writable
    let storage_ok = tokio::fs::metadata(&state.config.storage_path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);

    let all_ok = db_ok && storage_ok;
    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(ReadinessResponse {
            status: if all_ok {
                "ready".to_string()
            } else {
                "not_ready".to_string()
            },
            database: DatabaseHealth {
                connected: db_ok,
                pool_size,
                pool_idle,
                pool_active,
            },
            storage: if storage_ok {
                "ok".to_string()
            } else {
                "unavailable".to_string()
            },
            version: env!("CARGO_PKG_VERSION"),
        }),
    )
}

/// System statistics.
async fn stats(State(state): State<Arc<AppState>>, _auth: AuthUser) -> AppResult<Json<StatsResponse>> {
    let stats = state.stats_queries.get_stats().await?;
    Ok(Json(stats))
}

/// Prometheus-compatible metrics endpoint.
/// Renders all registered metrics (request latency, counts, DB pool stats, etc.)
/// plus static build info and rebuild status gauges.
/// Requires authentication to prevent unauthenticated enumeration of server state.
async fn metrics(State(state): State<Arc<AppState>>, _auth: AuthUser) -> impl IntoResponse {
    // Record current DB pool stats before rendering
    crate::metrics::record_pool_stats(&state.pool);

    // Record rebuild status as a gauge
    let in_progress = if state.rebuild_in_progress.load(Ordering::Relaxed) { 1.0 } else { 0.0 };
    ::metrics::gauge!("homorg_rebuild_in_progress").set(in_progress);

    let body = if let Some(ref handle) = state.metrics_handle {
        let mut output = handle.render();
        // Append static build info
        output.push_str(&format!(
            "\n# HELP homorg_build_info Static build information.\n\
             # TYPE homorg_build_info gauge\n\
             homorg_build_info{{version=\"{}\"}} 1\n",
            env!("CARGO_PKG_VERSION"),
        ));
        output
    } else {
        "# metrics recorder not installed\n".to_string()
    };

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

/// Replay event store and rebuild the items projection table.
/// Long-running — returns 202 Accepted and processes in background.
async fn rebuild_projections(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<StatusCode> {
    auth.require_role("admin")?;

    // API-5: Guard against launching two simultaneous rebuilds.
    let was_running = state
        .rebuild_in_progress
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err();
    if was_running {
        return Err(AppError::Conflict("A projection rebuild is already in progress".into()));
    }

    // Spawn rebuild in background; guard clears the flag when it drops.
    let pool = state.pool.clone();
    let flag = Arc::clone(&state.rebuild_in_progress);
    tokio::spawn(async move {
        use crate::RebuildGuard;
        let _guard = RebuildGuard(flag);
        tracing::info!("Starting projection rebuild...");
        match Projector::rebuild_all(&pool).await {
            Ok((total, 0)) => tracing::info!("Projection rebuild complete: {total} events replayed, 0 skipped"),
            Ok((total, skipped)) => tracing::warn!(
                "Projection rebuild complete with errors: {total} events replayed, {skipped} skipped (deserialization failures)"
            ),
            Err(e) => tracing::error!("Projection rebuild failed: {e}"),
        }
        // _guard drops here, clearing rebuild_in_progress
    });

    Ok(StatusCode::ACCEPTED)
}

/// API-5: Poll whether a rebuild is currently running.
#[derive(Debug, Serialize)]
struct RebuildStatusResponse {
    in_progress: bool,
}

async fn rebuild_status(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<Json<RebuildStatusResponse>> {
    auth.require_role("admin")?;
    Ok(Json(RebuildStatusResponse {
        in_progress: state.rebuild_in_progress.load(Ordering::Relaxed),
    }))
}
