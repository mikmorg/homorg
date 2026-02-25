use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppResult;
use crate::models::item::ItemSummary;

/// Read-side query handler for search operations.
#[derive(Clone)]
pub struct SearchQueries {
    pool: PgPool,
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchParams {
    pub q: Option<String>,          // full-text / fuzzy query
    pub path: Option<String>,       // LTREE lquery pattern
    pub category: Option<String>,
    pub condition: Option<String>,
    pub container_id: Option<Uuid>, // restrict to subtree
    pub tags: Option<String>,       // comma-separated
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

        let rows = sqlx::query_as::<_, ItemSummary>(
            r#"
            SELECT id, system_barcode, name, category, is_container, container_path::text as container_path,
                   parent_id, condition, tags, is_deleted, created_at, updated_at
            FROM items
            WHERE is_deleted = FALSE
              -- Full-text search
              AND (
                  $1::text IS NULL
                  OR search_vector @@ plainto_tsquery('english', $1)
                  OR name % $1
                  OR name ILIKE '%' || $12 || '%'
              )
              -- LTREE path pattern
              AND ($2::text IS NULL OR container_path ~ $2::lquery)
              -- Structured filters
              AND ($3::text IS NULL OR category = $3)
              AND ($4::text IS NULL OR condition = $4)
              AND ($5::uuid IS NULL OR container_path <@ (SELECT container_path FROM items WHERE id = $5))
              AND ($6::text[] IS NULL OR tags @> $6)
              AND ($7::bool IS NULL OR is_container = $7)
              AND ($8::float8 IS NULL OR current_value >= $8)
              AND ($9::float8 IS NULL OR current_value <= $9)
              -- Cursor: keyset pagination on (created_at, id)
              AND (
                  $10::uuid IS NULL
                  OR (created_at, id) > (
                      SELECT created_at, id FROM items WHERE id = $10
                  )
              )
            ORDER BY
              CASE WHEN $1::text IS NOT NULL AND search_vector @@ plainto_tsquery('english', $1)
                   THEN ts_rank(search_vector, plainto_tsquery('english', $1))
                   ELSE 0
              END DESC,
              name ASC
            LIMIT $11
            "#,
        )
        .bind(&params.q)         // $1: raw text for FTS + trigram
        .bind(&params.path)
        .bind(&params.category)
        .bind(&params.condition)
        .bind(params.container_id)
        .bind(tags.as_deref())
        .bind(params.is_container)
        .bind(params.min_value)
        .bind(params.max_value)
        .bind(params.cursor)
        .bind(limit)
        .bind(&ilike_q)          // $12: ILIKE-escaped text
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}
