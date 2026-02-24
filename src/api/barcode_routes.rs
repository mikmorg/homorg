use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::auth::middleware::AuthUser;
use crate::errors::AppResult;
use crate::models::barcode::{BarcodeResolution, GenerateBatchRequest, GeneratedBarcode};
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/generate", post(generate))
        .route("/generate-batch", post(generate_batch))
        .route("/resolve/{code}", get(resolve))
}

/// Generate a single new system barcode.
async fn generate(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<(StatusCode, Json<GeneratedBarcode>)> {
    let barcode = state.barcode_commands.generate_barcode().await?;
    Ok((StatusCode::CREATED, Json(barcode)))
}

/// Generate a batch of system barcodes.
async fn generate_batch(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(req): Json<GenerateBatchRequest>,
) -> AppResult<(StatusCode, Json<Vec<GeneratedBarcode>>)> {
    let barcodes = state.barcode_commands.generate_batch(req.count).await?;
    Ok((StatusCode::CREATED, Json(barcodes)))
}

/// Resolve a scanned barcode string.
async fn resolve(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(code): Path<String>,
) -> AppResult<Json<BarcodeResolution>> {
    let resolution = state.barcode_commands.resolve_barcode(&code).await?;
    Ok(Json(resolution))
}
