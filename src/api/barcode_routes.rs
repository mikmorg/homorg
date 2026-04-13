use axum::{
    body::Body,
    extract::{Json, Path, State},
    http::{header, StatusCode},
    response::Response,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::middleware::AuthUser;
use crate::constants::MAX_BARCODE_BATCH;
use crate::errors::{AppError, AppResult};
use crate::models::barcode::{BarcodeResolution, GenerateBatchRequest, GeneratedBarcode};
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/generate", post(generate))
        .route("/generate-batch", post(generate_batch))
        .route("/resolve/{code}", get(resolve))
}

/// PDF label generation routes — exposed separately so a tighter rate limit
/// can be applied in the top-level router (these are CPU/memory intensive).
pub fn pdf_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/labels", post(labels_pdf))
        .route("/preset-labels", post(preset_labels_pdf))
}

/// Generate a single new system barcode.
async fn generate(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<(StatusCode, Json<GeneratedBarcode>)> {
    auth.require_role("member")?;
    let barcode = state.barcode_commands.generate_barcode().await?;
    Ok((StatusCode::CREATED, Json(barcode)))
}

/// Generate a batch of system barcodes.
async fn generate_batch(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<GenerateBatchRequest>,
) -> AppResult<(StatusCode, Json<Vec<GeneratedBarcode>>)> {
    auth.require_role("member")?;
    if req.count == 0 || req.count > MAX_BARCODE_BATCH {
        return Err(AppError::BadRequest(format!(
            "count must be between 1 and {MAX_BARCODE_BATCH}"
        )));
    }
    let barcodes = state.barcode_commands.generate_batch(req.count).await?;
    Ok((StatusCode::CREATED, Json(barcodes)))
}

/// Resolve a scanned barcode string.
async fn resolve(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(code): Path<String>,
) -> AppResult<Json<BarcodeResolution>> {
    if code.is_empty() || code.len() > 256 {
        return Err(AppError::BadRequest(
            "Barcode must be between 1 and 256 characters".into(),
        ));
    }
    let resolution = state.barcode_commands.resolve_barcode(&code).await?;
    Ok(Json(resolution))
}

#[derive(Debug, Deserialize)]
struct LabelsPdfRequest {
    /// Generate this many new barcodes and print a label sheet for them.
    count: Option<u32>,
    /// Print a label sheet for these already-generated barcode strings.
    barcodes: Option<Vec<String>>,
}

/// Generate a PDF label sheet (3×10, OL25WX) with a Code128 barcode and barcode
/// number on each label.  Provide either `count` (reserves new barcodes in the
/// sequence) or `barcodes` (re-prints existing ones).
async fn labels_pdf(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<LabelsPdfRequest>,
) -> AppResult<Response> {
    auth.require_role("member")?;

    let barcodes: Vec<String> = match (req.count, req.barcodes) {
        (Some(count), None) => {
            if count == 0 || count > MAX_BARCODE_BATCH {
                return Err(AppError::BadRequest(format!(
                    "count must be between 1 and {MAX_BARCODE_BATCH}"
                )));
            }
            state
                .barcode_commands
                .generate_batch(count)
                .await?
                .into_iter()
                .map(|b| b.barcode)
                .collect()
        }
        (None, Some(barcodes)) => {
            if barcodes.is_empty() || barcodes.len() > MAX_BARCODE_BATCH as usize {
                return Err(AppError::BadRequest(format!(
                    "barcodes list must have 1 to {MAX_BARCODE_BATCH} entries"
                )));
            }
            // Only allow characters that are safe for both Code128 and LaTeX text.
            for b in &barcodes {
                if b.is_empty() || b.len() > 32 || !b.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
                {
                    return Err(AppError::BadRequest(format!(
                        "barcode \"{b}\" contains invalid characters or exceeds 32 chars"
                    )));
                }
            }
            barcodes
        }
        _ => {
            return Err(AppError::BadRequest(
                "provide either `count` or `barcodes`, not both".into(),
            ))
        }
    };

    let description = if barcodes.len() == 1 {
        format!("Labels | {} | 1 barcode | {}", barcodes[0], chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"))
    } else {
        format!(
            "Labels | {} .. {} | {} barcodes | {}",
            barcodes[0],
            barcodes[barcodes.len() - 1],
            barcodes.len(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
        )
    };
    let pdf = crate::label_gen::generate_label_pdf(&barcodes, &description).await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"labels.pdf\"")
        .body(Body::from(pdf))
        .unwrap())
}

#[derive(Debug, Deserialize)]
struct PresetLabelsPdfRequest {
    count: u32,
    is_container: bool,
    container_type_id: Option<uuid::Uuid>,
}

/// Generate preset barcode labels — each barcode is pre-registered as a container
/// or item so the stocker can auto-create the record on first scan without prompting
/// for a name (the barcode string becomes the default name).
async fn preset_labels_pdf(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<PresetLabelsPdfRequest>,
) -> AppResult<Response> {
    auth.require_role("member")?;

    if req.count == 0 || req.count > MAX_BARCODE_BATCH {
        return Err(AppError::BadRequest(format!(
            "count must be between 1 and {MAX_BARCODE_BATCH}"
        )));
    }

    // Validate container_type_id if provided.
    if let Some(type_id) = req.container_type_id {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM container_types WHERE id = $1)")
            .bind(type_id)
            .fetch_one(&state.pool)
            .await?;
        if !exists {
            return Err(AppError::NotFound(format!("Container type {type_id} not found")));
        }
    }

    // Generate barcodes and insert presets atomically — all in one transaction so
    // the sequence advance and preset rows are never out of sync.
    let mut tx = state.pool.begin().await?;
    let generated = state.barcode_commands.generate_batch_in_tx(&mut tx, req.count).await?;
    let barcodes: Vec<String> = generated.into_iter().map(|b| b.barcode).collect();

    for barcode in &barcodes {
        sqlx::query(
            "INSERT INTO barcode_presets (barcode, is_container, container_type_id) \
             VALUES ($1, $2, $3) ON CONFLICT (barcode) DO NOTHING",
        )
        .bind(barcode)
        .bind(req.is_container)
        .bind(req.container_type_id)
        .execute(&mut *tx)
        .await?;
    }
    state.event_store.commit_and_notify(tx).await?;

    let preset_kind = if req.is_container { "container" } else { "item" };
    let description = format!(
        "Preset {} labels | {} .. {} | {} barcodes | {}",
        preset_kind,
        barcodes[0],
        barcodes[barcodes.len() - 1],
        barcodes.len(),
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
    );
    let pdf = crate::label_gen::generate_label_pdf(&barcodes, &description).await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"preset-labels.pdf\"",
        )
        .body(Body::from(pdf))
        .unwrap())
}
