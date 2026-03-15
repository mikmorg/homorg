//! Shared query helpers used by multiple query modules.

use sqlx::PgPool;

use crate::errors::AppResult;
use crate::models::item::AncestorEntry;

/// Resolve ancestor breadcrumbs from an LTREE path string.
/// Uses a single batch query instead of N+1 per-label queries.
pub async fn resolve_ancestors(
    pool: &PgPool,
    path: &Option<String>,
) -> AppResult<Vec<AncestorEntry>> {
    let path_str = match path {
        Some(p) => p,
        None => return Ok(vec![]),
    };

    let labels: Vec<&str> = path_str.split('.').collect();
    let labels_owned: Vec<String> = labels.iter().map(|s| s.to_string()).collect();

    // Single batch query for all ancestor node_ids
    let rows = sqlx::query_as::<_, (uuid::Uuid, Option<String>, Option<String>, String)>(
        "SELECT id, system_barcode, name, node_id FROM items WHERE node_id = ANY($1)",
    )
    .bind(&labels_owned)
    .fetch_all(pool)
    .await?;

    // Build lookup map and reorder by path position
    let lookup: std::collections::HashMap<&str, &(uuid::Uuid, Option<String>, Option<String>, String)> =
        rows.iter().map(|r| (r.3.as_str(), r)).collect();

    let mut ancestors = Vec::with_capacity(labels.len());
    for (depth, label) in labels.iter().enumerate() {
        if let Some((id, barcode, name, node_id)) = lookup.get(label) {
            ancestors.push(AncestorEntry {
                id: *id,
                system_barcode: barcode.clone(),
                name: name.clone(),
                node_id: node_id.clone(),
                depth,
            });
        }
    }

    Ok(ancestors)
}
