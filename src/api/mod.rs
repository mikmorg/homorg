pub mod auth_routes;
pub mod barcode_routes;
pub mod category_routes;
pub mod container_routes;
pub mod container_type_routes;
pub mod item_routes;
pub mod search_routes;
pub mod stocker_routes;
pub mod system_routes;
pub mod tag_routes;
pub mod undo_routes;
pub mod user_routes;

use crate::config::AppConfig;
use crate::AppState;
use axum::Router;
use std::{net::IpAddr, sync::Arc};
use tower::util::option_layer;
use tower_governor::{governor::GovernorConfigBuilder, key_extractor::KeyExtractor, GovernorError, GovernorLayer};
use tower_http::services::ServeDir;

// ── SEC-6: Client IP extraction that respects reverse-proxy headers ──────────
/// Extracts the originating client IP from `X-Forwarded-For` (first entry) when
/// present, falling back to the direct peer address.  This is safe when the
/// service is deployed behind a trusted reverse proxy that *prepends* – rather
/// than appends – the real client IP.  The first value in X-Forwarded-For is the
/// one set by the outermost trusted proxy and cannot be spoofed by the client.
/// WARNING: Enable this only when behind a proxy that removes/replaces the header.
#[derive(Clone)]
struct ClientIpKeyExtractor;

impl KeyExtractor for ClientIpKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        // Check X-Forwarded-For first (first hop = client)
        if let Some(xff) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first) = xff.split(',').next() {
                let trimmed = first.trim();
                if trimmed.parse::<IpAddr>().is_ok() {
                    return Ok(trimmed.to_string());
                }
            }
        }
        // Fall back to X-Real-IP (set by nginx)
        if let Some(xri) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
            if xri.trim().parse::<IpAddr>().is_ok() {
                return Ok(xri.trim().to_string());
            }
        }
        // Last resort: peer address from axum's ConnectInfo (available when server uses
        // into_make_service_with_connect_info).
        if let Some(conn_info) = req
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        {
            return Ok(conn_info.0.ip().to_string());
        }
        Err(GovernorError::UnableToExtractKey)
    }
}

/// Build the complete API router with all v1 routes.
pub fn build_router(state: Arc<AppState>, config: &AppConfig) -> Router {
    // Rate limiting is opt-in: disabled unless RATE_LIMIT_RPS is explicitly set.
    let (auth_layer, api_layer, pdf_layer) = if config.rate_limit_enabled {
        let auth_conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(config.rate_limit_rps)
                .burst_size(config.rate_limit_burst)
                .key_extractor(ClientIpKeyExtractor)
                .finish()
                .expect("Failed to build auth rate limiter config"),
        );
        let api_conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(config.rate_limit_rps * 10)
                .burst_size(config.rate_limit_burst * 5)
                .key_extractor(ClientIpKeyExtractor)
                .finish()
                .expect("Failed to build API rate limiter config"),
        );
        // PDF label generation is CPU/memory intensive — 1 RPS with burst of 5.
        let pdf_conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(1)
                .burst_size(5)
                .key_extractor(ClientIpKeyExtractor)
                .finish()
                .expect("Failed to build PDF rate limiter config"),
        );
        (
            Some(GovernorLayer::new(auth_conf)),
            Some(GovernorLayer::new(api_conf)),
            Some(GovernorLayer::new(pdf_conf)),
        )
    } else {
        (None, None, None)
    };

    let api_v1 = Router::new()
        .nest("/auth", auth_routes::router().layer(option_layer(auth_layer)))
        .nest("/items", item_routes::router())
        .nest("/containers", container_routes::router())
        .nest(
            "/barcodes",
            barcode_routes::router().merge(barcode_routes::pdf_routes().layer(option_layer(pdf_layer))),
        )
        .nest("/stocker", stocker_routes::router())
        .nest("/search", search_routes::router())
        .nest("/undo", undo_routes::router())
        .nest("/users", user_routes::router())
        .nest("/container-types", container_type_routes::router())
        .nest("/tags", tag_routes::router())
        .nest("/categories", category_routes::router())
        .merge(system_routes::router())
        .layer(option_layer(api_layer));

    Router::new()
        .nest("/api/v1", api_v1)
        // Images are household inventory photos — not sensitive. No auth required
        // so standard browser <img src> tags work without fetch+blob workarounds.
        .nest_service("/files", ServeDir::new(&config.storage_path))
        .with_state(state)
}
