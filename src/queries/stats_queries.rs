use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::errors::{AppError, AppResult};

/// Read-side query handler for system statistics.
#[derive(Clone)]
pub struct StatsQueries {
    pool: PgPool,
}

/// Aggregate system statistics.
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_items: i64,
    pub total_containers: i64,
    pub total_events: i64,
    pub total_users: i64,
    pub items_by_category: Vec<CategoryCount>,
    pub items_by_condition: Vec<ConditionCount>,
}

/// Count of items in a given category.
#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryCount {
    pub category: Option<String>,
    pub count: i64,
}

/// Count of items in a given condition.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConditionCount {
    pub condition: Option<String>,
    pub count: i64,
}

impl StatsQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Gather all system statistics in a single round-trip using a CTE.
    ///
    /// The four scalar counts and the two aggregated breakdowns are fetched
    /// in one query. `json_agg` collects the multi-row subqueries into JSON
    /// arrays that are then deserialized into the typed response structs.
    pub async fn get_stats(&self) -> AppResult<StatsResponse> {
        #[derive(sqlx::FromRow)]
        struct StatsRawRow {
            total_items: i64,
            total_containers: i64,
            total_events: i64,
            total_users: i64,
            items_by_category: serde_json::Value,
            items_by_condition: serde_json::Value,
        }

        let row = sqlx::query_as::<_, StatsRawRow>(
            r#"
            WITH
            item_counts AS (
                SELECT
                    COUNT(*) FILTER (WHERE is_deleted = FALSE AND is_container = FALSE) AS total_items,
                    COUNT(*) FILTER (WHERE is_deleted = FALSE AND is_container = TRUE)  AS total_containers
                FROM items
            ),
            ev  AS (SELECT COUNT(*) AS total_events FROM event_store),
            usr AS (SELECT COUNT(*) AS total_users  FROM users WHERE is_active = TRUE),
            by_cat AS (
                SELECT COALESCE(json_agg(row_order), '[]'::json) AS data
                FROM (
                    SELECT c.name AS category, COUNT(i.id) AS count
                    FROM items i
                    LEFT JOIN categories c ON c.id = i.category_id
                    WHERE i.is_deleted = FALSE
                    GROUP BY c.name
                    ORDER BY count DESC
                    LIMIT 20
                ) row_order
            ),
            by_cond AS (
                SELECT COALESCE(json_agg(row_order), '[]'::json) AS data
                FROM (
                    SELECT condition, COUNT(*) AS count
                    FROM items
                    WHERE is_deleted = FALSE
                    GROUP BY condition
                    ORDER BY count DESC
                ) row_order
            )
            SELECT
                item_counts.total_items,
                item_counts.total_containers,
                ev.total_events,
                usr.total_users,
                by_cat.data  AS items_by_category,
                by_cond.data AS items_by_condition
            FROM item_counts, ev, usr, by_cat, by_cond
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let items_by_category: Vec<CategoryCount> =
            serde_json::from_value(row.items_by_category).map_err(|e| {
                AppError::Internal(format!("Failed to deserialize items_by_category: {e}"))
            })?;

        let items_by_condition: Vec<ConditionCount> =
            serde_json::from_value(row.items_by_condition).map_err(|e| {
                AppError::Internal(format!("Failed to deserialize items_by_condition: {e}"))
            })?;

        Ok(StatsResponse {
            total_items: row.total_items,
            total_containers: row.total_containers,
            total_events: row.total_events,
            total_users: row.total_users,
            items_by_category,
            items_by_condition,
        })
    }
}
