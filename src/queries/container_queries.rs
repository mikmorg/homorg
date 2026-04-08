use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::item::{ContainerStats, ItemSummary};
use crate::queries::common::resolve_ancestors;

// ── Base SELECT for slim ItemSummary rows ───────────────────────────────────
pub(crate) const ITEM_SUMMARY_SELECT: &str = r#"
    i.id,
    i.system_barcode,
    i.name,
    cat.name AS category,
    i.is_container,
    i.container_path::text AS container_path,
    i.parent_id,
    i.condition,
    COALESCE(ARRAY(
        SELECT t.name FROM item_tags it2
        JOIN tags t ON t.id = it2.tag_id
        WHERE it2.item_id = i.id
        ORDER BY t.name
    ), ARRAY[]::text[]) AS tags,
    i.is_deleted,
    i.created_at,
    i.updated_at
FROM items i
LEFT JOIN categories cat ON cat.id = i.category_id
"#;

/// Read-side query handler for container operations.
#[derive(Clone)]
pub struct ContainerQueries {
    pool: PgPool,
}

impl ContainerQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get direct children of a container, paginated with keyset cursor.
    pub async fn get_children(
        &self,
        container_id: Uuid,
        cursor: Option<Uuid>,
        limit: i64,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
    ) -> AppResult<Vec<ItemSummary>> {
        // Validate container exists
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM items WHERE id = $1 AND is_container = TRUE AND is_deleted = FALSE)",
        )
        .bind(container_id)
        .fetch_one(&self.pool)
        .await?;

        if !exists {
            return Err(AppError::NotFound(format!("Container {container_id} not found")));
        }

        let order_col = match sort_by.unwrap_or("name") {
            "name" => "i.name",
            "created_at" => "i.created_at",
            "updated_at" => "i.updated_at",
            "category" => "cat.name",
            "system_barcode" => "i.system_barcode",
            _ => "i.name",
        };
        let order_dir = if sort_dir.unwrap_or("asc") == "desc" {
            "DESC"
        } else {
            "ASC"
        };
        // CB-2: Cursor comparison operator must match sort direction.
        let cursor_op = if order_dir == "DESC" { "<" } else { ">" };

        // CB-2: Keyset cursor — align column aliases with ORDER BY.
        let cursor_subquery = match sort_by.unwrap_or("name") {
            "created_at" | "updated_at" => {
                let col = sort_by.unwrap_or("created_at");
                format!("OR (i.{col}, i.id) {cursor_op} (SELECT {col}, id FROM items WHERE id = $2)")
            }
            _ => {
                // The inner subquery must use the `i2` alias (the cursor row), not the
                // outer `i` alias (the current row).  Without this replacement, e.g.
                // `i.name` in the subquery would be a correlated reference to the outer
                // row, making the comparison always evaluate to FALSE and producing an
                // empty page 2+  for any name/barcode/category sort order.
                let inner_col = order_col.replace("i.", "i2.");
                format!(
                    "OR (COALESCE({order_col}, ''), i.id::text) {cursor_op} \
                     (SELECT COALESCE({inner_col}, ''), i2.id::text FROM items i2 \
                      LEFT JOIN categories cat ON cat.id = i2.category_id \
                      WHERE i2.id = $2)"
                )
            }
        };

        let query = format!(
            r#"
            SELECT {ITEM_SUMMARY_SELECT}
            WHERE i.parent_id = $1 AND i.is_deleted = FALSE
              AND (
                  $2::uuid IS NULL
                  {cursor_subquery}
              )
            ORDER BY COALESCE({order_col}, '') {order_dir}, i.id {order_dir}
            LIMIT $3
            "#
        );

        let rows = sqlx::query_as::<_, ItemSummary>(&query)
            .bind(container_id)
            .bind(cursor)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }

    /// Get all descendants via LTREE subtree query, with optional depth limit.
    pub async fn get_descendants(
        &self,
        container_id: Uuid,
        max_depth: Option<i32>,
        limit: i64,
    ) -> AppResult<Vec<ItemSummary>> {
        let container_path: Option<String> = sqlx::query_scalar(
            "SELECT container_path::text FROM items WHERE id = $1 AND is_container = TRUE AND is_deleted = FALSE",
        )
        .bind(container_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Container {container_id} not found")))?;

        let path = container_path.ok_or_else(|| AppError::Internal("Container has no path".into()))?;

        let path_depth = path.split('.').count() as i32;

        let rows = if let Some(max_d) = max_depth {
            let sql = format!(
                r#"
                SELECT {ITEM_SUMMARY_SELECT}
                WHERE i.container_path <@ $1::ltree
                  AND i.id != $2
                  AND i.is_deleted = FALSE
                  AND nlevel(i.container_path) <= $3
                ORDER BY i.container_path
                LIMIT $4
                "#
            );
            sqlx::query_as::<_, ItemSummary>(&sql)
                .bind(&path)
                .bind(container_id)
                .bind(path_depth + max_d)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
        } else {
            let sql = format!(
                r#"
                SELECT {ITEM_SUMMARY_SELECT}
                WHERE i.container_path <@ $1::ltree
                  AND i.id != $2
                  AND i.is_deleted = FALSE
                ORDER BY i.container_path
                LIMIT $3
                "#
            );
            sqlx::query_as::<_, ItemSummary>(&sql)
                .bind(&path)
                .bind(container_id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
        };

        Ok(rows)
    }

    /// Get ancestor breadcrumb path for a container.
    pub async fn get_ancestors(&self, container_id: Uuid) -> AppResult<Vec<crate::models::item::AncestorEntry>> {
        let path: Option<String> =
            sqlx::query_scalar("SELECT container_path::text FROM items WHERE id = $1 AND is_deleted = FALSE")
                .bind(container_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Item {container_id} not found")))?;

        resolve_ancestors(&self.pool, &path).await
    }

    /// Get container statistics.
    /// DB-2: Single CTE query instead of 6 separate round-trips.
    pub async fn get_stats(&self, container_id: Uuid) -> AppResult<ContainerStats> {
        let path: Option<String> = sqlx::query_scalar(
            "SELECT container_path::text FROM items WHERE id = $1 AND is_container = TRUE AND is_deleted = FALSE",
        )
        .bind(container_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Container {container_id} not found")))?;

        let path = path.ok_or_else(|| AppError::Internal("Container has no path".into()))?;

        // Single CTE. max_capacity_cc and max_weight_grams now live in container_properties.
        let row: (i64, i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>) = sqlx::query_as(
            r#"
            WITH
              container AS (
                SELECT cp.max_capacity_cc::float8  AS max_cap,
                       cp.max_weight_grams::float8 AS max_weight
                FROM items i
                LEFT JOIN container_properties cp ON cp.item_id = i.id
                WHERE i.id = $1
              ),
              children AS (
                SELECT
                  COUNT(*)                         AS child_count,
                  SUM(weight_grams::float8)        AS total_weight,
                  SUM(
                    COALESCE((dimensions->>'width_cm')::float8,  0) *
                    COALESCE((dimensions->>'height_cm')::float8, 0) *
                    COALESCE((dimensions->>'depth_cm')::float8,  0)
                  )                                AS capacity_used
                FROM items
                WHERE parent_id = $1 AND is_deleted = FALSE
              ),
              descendants AS (
                SELECT COUNT(*) AS desc_count
                FROM items
                WHERE container_path <@ $2::ltree AND id != $1 AND is_deleted = FALSE
              )
            SELECT
              children.child_count,
              descendants.desc_count,
              children.total_weight,
              children.capacity_used,
              container.max_cap,
              container.max_weight
            FROM container, children, descendants
            "#,
        )
        .bind(container_id)
        .bind(&path)
        .fetch_one(&self.pool)
        .await?;

        let (child_count, descendant_count, total_weight, capacity_used, max_cap, _max_weight) = row;

        let utilization_pct = match (capacity_used, max_cap) {
            (Some(used), Some(max)) if max > 0.0 => Some((used / max) * 100.0),
            _ => None,
        };

        Ok(ContainerStats {
            child_count,
            descendant_count,
            total_weight_grams: total_weight,
            capacity_used_cc: capacity_used,
            max_capacity_cc: max_cap,
            utilization_pct,
        })
    }
}
