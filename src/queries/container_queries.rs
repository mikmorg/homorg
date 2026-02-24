use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::item::{AncestorEntry, ContainerStats, ItemSummary};

/// Read-side query handler for container operations.
#[derive(Clone)]
pub struct ContainerQueries {
    pool: PgPool,
}

impl ContainerQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get direct children of a container, paginated.
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
            "name" => "name",
            "created_at" => "created_at",
            "updated_at" => "updated_at",
            "category" => "category",
            "system_barcode" => "system_barcode",
            _ => "name",
        };
        let order_dir = if sort_dir.unwrap_or("asc") == "desc" { "DESC" } else { "ASC" };

        let query = format!(
            r#"
            SELECT id, system_barcode, name, category, is_container, container_path::text as container_path,
                   parent_id, condition, tags, is_deleted, created_at, updated_at
            FROM items
            WHERE parent_id = $1 AND is_deleted = FALSE
              AND ($2::uuid IS NULL OR id > $2)
            ORDER BY {order_col} {order_dir}
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

        let path = container_path
            .ok_or_else(|| AppError::Internal("Container has no path".into()))?;

        let path_depth = path.split('.').count() as i32;

        let rows = if let Some(max_d) = max_depth {
            sqlx::query_as::<_, ItemSummary>(
                r#"
                SELECT id, system_barcode, name, category, is_container, container_path::text as container_path,
                       parent_id, condition, tags, is_deleted, created_at, updated_at
                FROM items
                WHERE container_path <@ $1::ltree
                  AND id != $2
                  AND is_deleted = FALSE
                  AND nlevel(container_path) <= $3
                ORDER BY container_path
                LIMIT $4
                "#,
            )
            .bind(&path)
            .bind(container_id)
            .bind(path_depth + max_d)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ItemSummary>(
                r#"
                SELECT id, system_barcode, name, category, is_container, container_path::text as container_path,
                       parent_id, condition, tags, is_deleted, created_at, updated_at
                FROM items
                WHERE container_path <@ $1::ltree
                  AND id != $2
                  AND is_deleted = FALSE
                ORDER BY container_path
                LIMIT $3
                "#,
            )
            .bind(&path)
            .bind(container_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows)
    }

    /// Get ancestor breadcrumb path for a container.
    pub async fn get_ancestors(&self, container_id: Uuid) -> AppResult<Vec<AncestorEntry>> {
        let path: Option<String> = sqlx::query_scalar(
            "SELECT container_path::text FROM items WHERE id = $1 AND is_deleted = FALSE",
        )
        .bind(container_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {container_id} not found")))?;

        let path_str = match path {
            Some(p) => p,
            None => return Ok(vec![]),
        };

        let labels: Vec<&str> = path_str.split('.').collect();
        let mut ancestors = Vec::with_capacity(labels.len());

        for (depth, label) in labels.iter().enumerate() {
            let row = sqlx::query_as::<_, (Uuid, String, Option<String>, String)>(
                "SELECT id, system_barcode, name, ltree_label FROM items WHERE ltree_label = $1",
            )
            .bind(label)
            .fetch_optional(&self.pool)
            .await?;

            if let Some((id, barcode, name, ltree_label)) = row {
                ancestors.push(AncestorEntry {
                    id,
                    system_barcode: barcode,
                    name,
                    ltree_label,
                    depth,
                });
            }
        }

        Ok(ancestors)
    }

    /// Get container statistics.
    pub async fn get_stats(&self, container_id: Uuid) -> AppResult<ContainerStats> {
        let path: Option<String> = sqlx::query_scalar(
            "SELECT container_path::text FROM items WHERE id = $1 AND is_container = TRUE AND is_deleted = FALSE",
        )
        .bind(container_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Container {container_id} not found")))?;

        let path = path.ok_or_else(|| AppError::Internal("Container has no path".into()))?;

        let child_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM items WHERE parent_id = $1 AND is_deleted = FALSE",
        )
        .bind(container_id)
        .fetch_one(&self.pool)
        .await?;

        let descendant_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM items WHERE container_path <@ $1::ltree AND id != $2 AND is_deleted = FALSE",
        )
        .bind(&path)
        .bind(container_id)
        .fetch_one(&self.pool)
        .await?;

        let total_weight: Option<f64> = sqlx::query_scalar(
            "SELECT SUM(weight_grams::float8) FROM items WHERE parent_id = $1 AND is_deleted = FALSE AND weight_grams IS NOT NULL",
        )
        .bind(container_id)
        .fetch_one(&self.pool)
        .await?;

        // Get container's max capacity
        let (max_cap, _max_weight): (Option<f64>, Option<f64>) = sqlx::query_as(
            "SELECT max_capacity_cc::float8, max_weight_grams::float8 FROM items WHERE id = $1",
        )
        .bind(container_id)
        .fetch_one(&self.pool)
        .await?;

        // Estimate volume usage from children's dimensions (simplified: sum of w*h*d)
        let capacity_used: Option<f64> = sqlx::query_scalar(
            r#"
            SELECT SUM(
                COALESCE((dimensions->>'width_cm')::float8, 0) *
                COALESCE((dimensions->>'height_cm')::float8, 0) *
                COALESCE((dimensions->>'depth_cm')::float8, 0)
            )
            FROM items
            WHERE parent_id = $1 AND is_deleted = FALSE AND dimensions IS NOT NULL
            "#,
        )
        .bind(container_id)
        .fetch_one(&self.pool)
        .await?;

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
