use serde::Serialize;
use sqlx::PgPool;

use crate::errors::AppResult;

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
#[derive(Debug, Serialize)]
pub struct CategoryCount {
    pub category: Option<String>,
    pub count: i64,
}

/// Count of items in a given condition.
#[derive(Debug, Serialize)]
pub struct ConditionCount {
    pub condition: Option<String>,
    pub count: i64,
}

impl StatsQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Gather all system statistics in a single call.
    pub async fn get_stats(&self) -> AppResult<StatsResponse> {
        let total_items: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM items WHERE is_deleted = FALSE AND is_container = FALSE",
        )
        .fetch_one(&self.pool)
        .await?;

        let total_containers: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM items WHERE is_deleted = FALSE AND is_container = TRUE",
        )
        .fetch_one(&self.pool)
        .await?;

        let total_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_store")
            .fetch_one(&self.pool)
            .await?;

        let total_users: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_active = TRUE")
                .fetch_one(&self.pool)
                .await?;

        let items_by_category: Vec<CategoryCount> = sqlx::query_as::<_, (Option<String>, i64)>(
            "SELECT category, COUNT(*) as count FROM items WHERE is_deleted = FALSE GROUP BY category ORDER BY count DESC LIMIT 20",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(category, count)| CategoryCount { category, count })
        .collect();

        let items_by_condition: Vec<ConditionCount> = sqlx::query_as::<_, (Option<String>, i64)>(
            "SELECT condition, COUNT(*) as count FROM items WHERE is_deleted = FALSE GROUP BY condition ORDER BY count DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(condition, count)| ConditionCount { condition, count })
        .collect();

        Ok(StatsResponse {
            total_items,
            total_containers,
            total_events,
            total_users,
            items_by_category,
            items_by_condition,
        })
    }
}
