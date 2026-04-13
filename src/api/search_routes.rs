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
/// API-5: Maximum length for LTREE lquery path pattern.
const MAX_SEARCH_PATH_LEN: usize = 500;

/// API-5: Only allow safe LTREE lquery characters (alphanumeric, underscore,
/// dot, and the star wildcard).  Reject all other lquery meta-characters
/// ({, }, !, @, |, quantifiers) to prevent computationally expensive patterns.
fn is_safe_lquery(path: &str) -> bool {
    path.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'*')
}

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
    // chars().count() counts Unicode scalars, matching the user-visible "characters" limit.
    // Normalize empty-string q to None so `q=""` behaves identically to omitting q.
    let params = SearchParams {
        q: params.q.filter(|q| !q.trim().is_empty()),
        ..params
    };
    if let Some(ref q) = params.q {
        if q.chars().count() > MAX_SEARCH_QUERY_LEN {
            return Err(AppError::BadRequest(format!(
                "Search query exceeds maximum length of {MAX_SEARCH_QUERY_LEN} characters"
            )));
        }
    }
    // API-5: Validate lquery path to prevent DoS via expensive patterns.
    if let Some(ref path) = params.path {
        if path.len() > MAX_SEARCH_PATH_LEN {
            return Err(AppError::BadRequest(format!(
                "Path pattern exceeds maximum length of {MAX_SEARCH_PATH_LEN} bytes"
            )));
        }
        if !is_safe_lquery(path) {
            return Err(AppError::BadRequest(
                "Path pattern contains invalid characters; only alphanumeric, '_', '.', and '*' are allowed".into(),
            ));
        }
    }
    let results = state.search_queries.search(&params).await?;
    Ok(Json(results))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_lquery_accepts_simple_path() {
        assert!(is_safe_lquery("root.box.shelf"));
    }

    #[test]
    fn safe_lquery_accepts_wildcard() {
        assert!(is_safe_lquery("root.*.shelf"));
    }

    #[test]
    fn safe_lquery_accepts_underscore() {
        assert!(is_safe_lquery("my_box.item_1"));
    }

    #[test]
    fn safe_lquery_rejects_pipe() {
        assert!(!is_safe_lquery("root|admin"));
    }

    #[test]
    fn safe_lquery_rejects_braces() {
        assert!(!is_safe_lquery("root.{a,b}"));
    }

    #[test]
    fn safe_lquery_rejects_bang() {
        assert!(!is_safe_lquery("!root"));
    }

    #[test]
    fn safe_lquery_rejects_at() {
        assert!(!is_safe_lquery("root@3"));
    }

    #[test]
    fn safe_lquery_accepts_empty() {
        assert!(is_safe_lquery(""));
    }

    #[test]
    fn safe_lquery_rejects_space() {
        assert!(!is_safe_lquery("root box"));
    }

    #[test]
    fn safe_lquery_rejects_semicolon() {
        assert!(!is_safe_lquery("root;DROP"));
    }
}
