use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;

use crate::auth::middleware::AuthUser;
use crate::errors::{AppError, AppResult};
use crate::models::item::ItemSummary;
use crate::queries::search_queries::SearchParams;
use crate::AppState;

/// API-4: Maximum allowed length for the search query string.
const MAX_SEARCH_QUERY_LEN: usize = 500;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(search))
}

/// Combined search: full-text + trigram + LTREE path + structured filters.
async fn search(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Query(params): Query<SearchParams>,
) -> AppResult<Json<Vec<ItemSummary>>> {
    // API-4: Reject overly long query strings to prevent DoS via expensive full-text ops.
    if let Some(ref q) = params.q {
        if q.len() > MAX_SEARCH_QUERY_LEN {
            return Err(AppError::BadRequest(format!(
                "Search query exceeds maximum length of {MAX_SEARCH_QUERY_LEN} characters"
            )));
        }
    }
    let results = state.search_queries.search(&params).await?;
    Ok(Json(results))
}
