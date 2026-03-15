use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppResult;
use crate::models::item::ItemSummary;
use crate::queries::container_queries::ITEM_SUMMARY_SELECT;

/// Read-side query handler for search operations.
#[derive(Clone)]
pub struct SearchQueries {
    pool: PgPool,
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchParams {
    pub q: Option<String>,          // full-text / fuzzy query
    pub path: Option<String>,       // LTREE lquery pattern
    pub category: Option<String>,   // category name (exact)
    pub condition: Option<String>,
    pub container_id: Option<Uuid>, // restrict to subtree
    pub tags: Option<String>,       // comma-separated tag names
    pub is_container: Option<bool>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub cursor: Option<Uuid>,
    pub limit: Option<i64>,
}

/// Escape ILIKE special characters (`\`, `%`, `_`) so user input is treated literally.
fn escape_ilike(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

impl SearchQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Combined search: full-text + trigram + LTREE path + structured filters.
    pub async fn search(&self, params: &SearchParams) -> AppResult<Vec<ItemSummary>> {
        let limit = params.limit.unwrap_or(50).min(200);

        // Parse tags
        let tags: Option<Vec<String>> = params.tags.as_ref().map(|t| {
            t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        });

        // Escape ILIKE wildcards so user input is literal
        let ilike_q: Option<String> = params.q.as_ref().map(|q| escape_ilike(q));

        // Tag filter: all provided tag names must be present on the item (AND semantics).
        // We use a NOT EXISTS + unnest approach so we don't need to join + group.
        // $6 is NULL → skip tag filter; non-NULL → all tags must match.
        let rows = sqlx::query_as::<_, ItemSummary>(
            &format!(r#"
            SELECT {ITEM_SUMMARY_SELECT}
            WHERE i.is_deleted = FALSE
              -- Full-text search
              AND (
                  $1::text IS NULL
                  OR i.search_vector @@ plainto_tsquery('english', $1)
                  OR i.name % $1
                  OR i.name ILIKE '%' || $12 || '%'
              )
              -- LTREE path pattern
              AND ($2::text IS NULL OR i.container_path ~ $2::lquery)
              -- Category filter (by name via JOIN)
              AND ($3::text IS NULL OR cat.name = $3)
              AND ($4::text IS NULL OR i.condition = $4)
              AND ($5::uuid IS NULL OR i.container_path <@ (SELECT container_path FROM items WHERE id = $5))
              -- Tag filter: all listed tags must be present
              AND ($6::text[] IS NULL OR NOT EXISTS (
                  SELECT 1 FROM unnest($6::text[]) AS required_tag
                  WHERE NOT EXISTS (
                      SELECT 1 FROM item_tags it3
                      JOIN tags tg ON tg.id = it3.tag_id
                      WHERE it3.item_id = i.id AND tg.name = required_tag
                  )
              ))
              AND ($7::bool IS NULL OR i.is_container = $7)
              AND ($8::float8 IS NULL OR i.current_value >= $8)
              AND ($9::float8 IS NULL OR i.current_value <= $9)
              -- Keyset cursor on (COALESCE(name,''), id)
              AND (
                  $10::uuid IS NULL
                  OR (COALESCE(i.name, ''), i.id) > (
                      SELECT COALESCE(name, ''), id FROM items WHERE id = $10
                  )
              )
            ORDER BY
              CASE WHEN $1::text IS NOT NULL AND i.search_vector @@ plainto_tsquery('english', $1)
                   THEN ts_rank(i.search_vector, plainto_tsquery('english', $1))
                   ELSE 0
              END DESC,
              COALESCE(i.name, '') ASC,
              i.id ASC
            LIMIT $11
            "#),
        )
        .bind(&params.q)         // $1: raw text for FTS + trigram
        .bind(&params.path)      // $2
        .bind(&params.category)  // $3
        .bind(&params.condition) // $4
        .bind(params.container_id) // $5
        .bind(tags.as_deref())   // $6
        .bind(params.is_container) // $7
        .bind(params.min_value)  // $8
        .bind(params.max_value)  // $9
        .bind(params.cursor)     // $10
        .bind(limit)             // $11
        .bind(&ilike_q)          // $12: ILIKE-escaped text
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}
