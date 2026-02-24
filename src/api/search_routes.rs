use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;

use crate::auth::middleware::AuthUser;
use crate::errors::AppResult;
use crate::models::item::ItemSummary;
use crate::queries::search_queries::SearchParams;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(search))
}

/// Combined search: full-text + trigram + LTREE path + structured filters.
async fn search(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Query(params): Query<SearchParams>,
) -> AppResult<Json<Vec<ItemSummary>>> {
    let results = state.search_queries.search(&params).await?;
    Ok(Json(results))
}
