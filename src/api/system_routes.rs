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

#[derive(Debug, Serialize)]
struct StatsResponse {
    total_items: i64,
    total_containers: i64,
    total_events: i64,
    total_users: i64,
    items_by_category: Vec<CategoryCount>,
    items_by_condition: Vec<ConditionCount>,
}

#[derive(Debug, Serialize)]
struct CategoryCount {
    category: Option<String>,
    count: i64,
}

#[derive(Debug, Serialize)]
struct ConditionCount {
    condition: Option<String>,
    count: i64,
}

/// System statistics.
async fn stats(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<Json<StatsResponse>> {
    let total_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM items WHERE is_deleted = FALSE AND is_container = FALSE",
    )
    .fetch_one(&state.pool)
    .await?;

    let total_containers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM items WHERE is_deleted = FALSE AND is_container = TRUE",
    )
    .fetch_one(&state.pool)
    .await?;

    let total_events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM event_store")
            .fetch_one(&state.pool)
            .await?;

    let total_users: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_active = TRUE")
            .fetch_one(&state.pool)
            .await?;

    let items_by_category: Vec<CategoryCount> = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT category, COUNT(*) as count FROM items WHERE is_deleted = FALSE GROUP BY category ORDER BY count DESC LIMIT 20",
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|(category, count)| CategoryCount { category, count })
    .collect();

    let items_by_condition: Vec<ConditionCount> = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT condition, COUNT(*) as count FROM items WHERE is_deleted = FALSE GROUP BY condition ORDER BY count DESC",
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|(condition, count)| ConditionCount { condition, count })
    .collect();

    Ok(Json(StatsResponse {
        total_items,
        total_containers,
        total_events,
        total_users,
        items_by_category,
        items_by_condition,
    }))
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
