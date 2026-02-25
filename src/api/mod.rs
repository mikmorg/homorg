pub mod auth_routes;
pub mod item_routes;
pub mod container_routes;
pub mod barcode_routes;
pub mod stocker_routes;
pub mod search_routes;
pub mod undo_routes;
pub mod user_routes;
pub mod system_routes;

use axum::Router;
use std::sync::Arc;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::services::ServeDir;
use crate::config::AppConfig;
use crate::AppState;

/// Build the complete API router with all v1 routes.
pub fn build_router(state: Arc<AppState>, config: &AppConfig) -> Router {
    // Rate limiter for auth endpoints (brute-force protection)
    let auth_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(state.config.rate_limit_rps)
            .burst_size(state.config.rate_limit_burst)
            .finish()
            .expect("Failed to build rate limiter config"),
    );

    // General API rate limiter (higher limits)
    let api_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(state.config.rate_limit_rps * 10)
            .burst_size(state.config.rate_limit_burst * 5)
            .finish()
            .expect("Failed to build API rate limiter config"),
    );

    let api_v1 = Router::new()
        .nest(
            "/auth",
            auth_routes::router().layer(GovernorLayer::new(auth_governor_conf)),
        )
        .nest("/items", item_routes::router())
        .nest("/containers", container_routes::router())
        .nest("/barcodes", barcode_routes::router())
        .nest("/stocker", stocker_routes::router())
        .nest("/search", search_routes::router())
        .nest("/undo", undo_routes::router())
        .nest("/users", user_routes::router())
        .merge(system_routes::router())
        .layer(GovernorLayer::new(api_governor_conf));

    Router::new()
        .nest("/api/v1", api_v1)
        // Serve uploaded files behind auth-free path but scoped to storage dir only
        // Files are UUID-named so not guessable; ServeDir already prevents traversal.
        // Health endpoint must remain unauthenticated for orchestrator probes.
        .nest_service("/files", ServeDir::new(&config.storage_path))
        .with_state(state)
}
