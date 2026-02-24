use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::auth::middleware::AuthUser;
use crate::errors::AppResult;
use crate::events::projector::Projector;
use crate::queries::stats_queries::StatsResponse;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/stats", get(stats))
        .route("/admin/rebuild-projections", post(rebuild_projections))
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    database: String,
}

/// Liveness check with DB connectivity status.
async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let db_status = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await;

    let (status, database) = match db_status {
        Ok(_) => ("ok".to_string(), "connected".to_string()),
        Err(e) => ("degraded".to_string(), format!("error: {e}")),
    };

    Json(HealthResponse { status, database })
}

/// System statistics.
async fn stats(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<Json<StatsResponse>> {
    let stats = state.stats_queries.get_stats().await?;
    Ok(Json(stats))
}

/// Replay event store and rebuild the items projection table.
/// Long-running — returns 202 Accepted and processes in background.
async fn rebuild_projections(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<StatusCode> {
    auth.require_role("admin")?;

    // Spawn rebuild in background
    let pool = state.pool.clone();
    tokio::spawn(async move {
        tracing::info!("Starting projection rebuild...");
        match Projector::rebuild_all(&pool).await {
            Ok(count) => tracing::info!("Projection rebuild complete: {count} events replayed"),
            Err(e) => tracing::error!("Projection rebuild failed: {e}"),
        }
    });

    Ok(StatusCode::ACCEPTED)
}
