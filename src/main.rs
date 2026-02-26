use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use axum::http::{header, Method};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use homorg::api;
use homorg::config::AppConfig;
use homorg::db;
use homorg::events::store::EventStore;
use homorg::storage::LocalStorage;
use homorg::AppState;

#[tokio::main]
async fn main() {
    // Load configuration first (needed for log format)
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env().expect("Failed to load configuration");

    // Initialize tracing (JSON format if LOG_FORMAT=json)
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("homorg=debug,info"));
    if config.log_format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }

    tracing::info!("Starting Homorg daemon on {}", config.listen_addr);

    // Create database pool and run migrations
    let pool = db::create_pool(&config).await.expect("Failed to create database pool");

    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Database migrations complete");

    // Initialize storage backend
    let storage = LocalStorage::new(&config.storage_path);
    storage.init().await.expect("Failed to initialize storage");
    let storage: Arc<dyn homorg::storage::StorageBackend> = Arc::new(storage);

    // Build shared state via constructor
    let event_store = EventStore::new(pool.clone());
    let state = Arc::new(AppState::new(config.clone(), pool, event_store, storage));

    // Run initial token cleanup and start periodic background task
    {
        let token_repo = state.token_repository.clone();
        // Purge expired tokens at startup
        match token_repo.purge_expired().await {
            Ok(n) if n > 0 => tracing::info!("Purged {n} expired tokens at startup"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "Failed to purge expired tokens at startup"),
        }
        match token_repo.purge_stale_revoked(7).await {
            Ok(n) if n > 0 => tracing::info!("Purged {n} stale revoked tokens at startup"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "Failed to purge stale revoked tokens at startup"),
        }
        // Periodic cleanup every 6 hours
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
            interval.tick().await; // skip immediate tick (already ran above)
            loop {
                interval.tick().await;
                if let Err(e) = token_repo.purge_expired().await {
                    tracing::warn!(error = %e, "Periodic token purge failed");
                }
                if let Err(e) = token_repo.purge_stale_revoked(7).await {
                    tracing::warn!(error = %e, "Periodic stale token purge failed");
                }
            }
        });
    }

    // Build CORS layer from config
    let cors = if config.cors_origins.len() == 1 && config.cors_origins[0] == "*" {
        CorsLayer::permissive()
    } else {
        let origins: Vec<_> = config
            .cors_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        // SEC-7: Restrict methods and headers for non-wildcard origins.
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                header::ACCEPT,
            ])
    };

    // Build router with compression, body limits, request ID
    // NOTE: /files is served inside the API router with auth (see api/mod.rs)
    // OP-2: Custom TraceLayer span includes the x-request-id so every log line for
    // a request can be correlated back to the HTTP call.
    let trace_layer = TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
        let request_id = request
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        tracing::span!(
            Level::INFO,
            "request",
            method = %request.method(),
            uri = %request.uri(),
            request_id = %request_id,
        )
    });

    let app = api::build_router(state, &config)
        .layer(DefaultBodyLimit::max(config.max_upload_bytes))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(CompressionLayer::new())
        .layer(trace_layer)
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(cors);

    // Start server with graceful shutdown
    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("Homorg daemon listening on {}", config.listen_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");

    tracing::info!("Homorg daemon shut down gracefully");
}

/// Wait for SIGINT (Ctrl-C) or SIGTERM, then return.
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received SIGINT, shutting down..."),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down..."),
    }
}
