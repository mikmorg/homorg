pub mod auth_routes;
pub mod item_routes;
pub mod container_routes;
pub mod barcode_routes;
pub mod stocker_routes;
pub mod search_routes;
pub mod undo_routes;
pub mod user_routes;
pub mod system_routes;

use axum::{
    extract::State,
    middleware::{self, Next},
    response::IntoResponse,
    Router,
};
use std::{net::IpAddr, sync::Arc};
use tower::ServiceBuilder;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::KeyExtractor, GovernorError, GovernorLayer,
};
use tower_http::services::ServeDir;
use crate::config::AppConfig;
use crate::errors::AppError;
use crate::AppState;

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
        if let Some(xff) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
        {
            if let Some(first) = xff.split(',').next() {
                let trimmed = first.trim();
                if trimmed.parse::<IpAddr>().is_ok() {
                    return Ok(trimmed.to_string());
                }
            }
        }
        // Fall back to X-Real-IP (set by nginx)
        if let Some(xri) = req
            .headers()
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
        {
            if xri.trim().parse::<IpAddr>().is_ok() {
                return Ok(xri.trim().to_string());
            }
        }
        // Last resort: peer address injected by axum's ConnectInfo (not always available)
        // tower_governor's default PeerIpKeyExtractor handles this; we return an error
        // here to fall through to the governor's built-in behaviour.
        Err(GovernorError::UnableToExtractKey)
    }
}

/// SEC-3: Middleware that validates a bearer token before serving uploaded files.
/// Checks the JWT signature and expiry; does NOT re-fetch the DB (to keep latency low for
/// static assets). Revoked tokens will be rejected on the next API call; this is an
/// acceptable trade-off since file URLs are not guessable (UUID-based paths).
async fn require_file_auth(
    State(state): State<Arc<AppState>>,
    request: axum::extract::Request,
    next: Next,
) -> impl IntoResponse {
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    match token {
        Some(t) => match crate::auth::jwt::decode_access_token(&t, &state.config.jwt_secret) {
            Ok(_) => next.run(request).await,
            Err(e) => e.into_response(),
        },
        None => AppError::Unauthorized.into_response(),
    }
}

/// Build the complete API router with all v1 routes.
pub fn build_router(state: Arc<AppState>, config: &AppConfig) -> Router {
    // Rate limiter for auth endpoints (brute-force protection)
    // SEC-6: Use ClientIpKeyExtractor so traffic behind a reverse proxy is rate-limited
    // per originating IP rather than per proxy IP.
    let auth_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(state.config.rate_limit_rps)
            .burst_size(state.config.rate_limit_burst)
            .key_extractor(ClientIpKeyExtractor)
            .finish()
            .expect("Failed to build rate limiter config"),
    );

    // General API rate limiter (higher limits)
    let api_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(state.config.rate_limit_rps * 10)
            .burst_size(state.config.rate_limit_burst * 5)
            .key_extractor(ClientIpKeyExtractor)
            .finish()
            .expect("Failed to build API rate limiter config"),
    );

    // SEC-3: Wrap ServeDir with a lightweight JWT auth check.
    let files_service = ServiceBuilder::new()
        .layer(middleware::from_fn_with_state(state.clone(), require_file_auth))
        .service(ServeDir::new(&config.storage_path));

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
        // SEC-3: /files now requires a valid bearer token.
        .nest_service("/files", files_service)
        .with_state(state)
}
