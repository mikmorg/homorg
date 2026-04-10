//! Prometheus metrics instrumentation.
//!
//! Installs a global metrics recorder and provides an Axum middleware layer
//! that tracks request latency and counts by method, path pattern, and status.

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::IntoResponse,
};
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::time::Instant;

/// Install the global Prometheus recorder and return a handle for rendering.
pub fn install_recorder() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder")
}

/// Axum middleware that records request duration, count, and status code.
pub async fn track_request(req: Request, next: Next) -> impl IntoResponse {
    let method = req.method().to_string();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|mp| mp.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    let start = Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed().as_secs_f64();

    let status = response.status().as_u16().to_string();
    let labels = [
        ("method", method),
        ("path", path),
        ("status", status),
    ];

    histogram!("http_request_duration_seconds", &labels).record(duration);
    counter!("http_requests_total", &labels).increment(1);

    response
}

/// Record DB pool utilization gauges. Call periodically or on each metrics scrape.
pub fn record_pool_stats(pool: &sqlx::PgPool) {
    let size = pool.size() as f64;
    let idle = pool.num_idle() as f64;
    gauge!("db_pool_size").set(size);
    gauge!("db_pool_idle").set(idle);
    gauge!("db_pool_active").set(size - idle);
}
