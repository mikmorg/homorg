use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::taxonomy::{
    Category, CreateCategoryRequest, CreateTagRequest, RenameTagRequest, Tag, UpdateCategoryRequest,
};

/// Read/write query handler for tags and categories.
#[derive(Clone)]
pub struct TaxonomyQueries {
    pool: PgPool,
}

impl TaxonomyQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Tags
    // ─────────────────────────────────────────────────────────────────────

    /// List all tags with their item counts (excludes soft-deleted items).
    pub async fn list_tags(&self) -> AppResult<Vec<Tag>> {
        let rows = sqlx::query_as::<_, Tag>(
            r#"
            SELECT t.id, t.name, t.created_at,
                   COUNT(it.item_id) FILTER (WHERE i.is_deleted = FALSE) AS item_count
            FROM tags t
            LEFT JOIN item_tags it ON it.tag_id = t.id
            LEFT JOIN items i ON i.id = it.item_id
            GROUP BY t.id, t.name, t.created_at
            ORDER BY t.name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get a single tag by ID.
    pub async fn get_tag_by_id(&self, id: Uuid) -> AppResult<Tag> {
        sqlx::query_as::<_, Tag>(
            r#"
            SELECT t.id, t.name, t.created_at,
                   COUNT(it.item_id) AS item_count
            FROM tags t
            LEFT JOIN item_tags it ON it.tag_id = t.id
            WHERE t.id = $1
            GROUP BY t.id
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Tag {id} not found")))
    }

    /// Get or create a tag by name (case-insensitive match on creation).
    /// Returns the tag ID.
    pub async fn get_or_create_tag_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        name: &str,
    ) -> AppResult<Uuid> {
        // Upsert: create if not exists, always return id
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO tags (name)
            VALUES ($1)
            ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
            RETURNING id
            "#,
        )
        .bind(name)
        .fetch_one(&mut **tx)
        .await?;
        Ok(id)
    }

    /// Create a new tag (fails if name already exists).
    pub async fn create_tag(&self, req: &CreateTagRequest) -> AppResult<Tag> {
        let row = sqlx::query_as::<_, Tag>(
            r#"
            INSERT INTO tags (name) VALUES ($1)
            RETURNING id, name, created_at, 0::bigint AS item_count
            "#,
        )
        .bind(&req.name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("uq_tags_name") {
                    return AppError::Conflict(format!("Tag '{}' already exists", req.name));
                }
            }
            AppError::Database(e)
        })?;
        Ok(row)
    }

    /// Rename a tag.  All items tagged with it automatically reflect the new name.
    pub async fn rename_tag(&self, id: Uuid, req: &RenameTagRequest) -> AppResult<Tag> {
        let row = sqlx::query_as::<_, Tag>(
            r#"
            UPDATE tags SET name = $1 WHERE id = $2
            RETURNING id, name, created_at,
                      (SELECT COUNT(*) FROM item_tags WHERE tag_id = $2) AS item_count
            "#,
        )
        .bind(&req.name)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("uq_tags_name") {
                    return AppError::Conflict(format!("Tag '{}' already exists", req.name));
                }
            }
            AppError::Database(e)
        })?
        .ok_or_else(|| AppError::NotFound(format!("Tag {id} not found")))?;
        Ok(row)
    }

    /// Delete a tag.  CASCADE removes all item_tags rows referencing it.
    pub async fn delete_tag(&self, id: Uuid) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM tags WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Tag {id} not found")));
        }
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Categories
    // ─────────────────────────────────────────────────────────────────────

    /// List all categories with item counts.
    pub async fn list_categories(&self) -> AppResult<Vec<Category>> {
        let rows = sqlx::query_as::<_, Category>(
            r#"
            SELECT c.id, c.name, c.description, c.parent_category_id,
                   c.created_at, c.updated_at,
                   COUNT(i.id) AS item_count
            FROM categories c
            LEFT JOIN items i ON i.category_id = c.id AND i.is_deleted = FALSE
            GROUP BY c.id
            ORDER BY c.name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get a single category by ID.
    pub async fn get_category_by_id(&self, id: Uuid) -> AppResult<Category> {
        sqlx::query_as::<_, Category>(
            r#"
            SELECT c.id, c.name, c.description, c.parent_category_id,
                   c.created_at, c.updated_at,
                   COUNT(i.id) AS item_count
            FROM categories c
            LEFT JOIN items i ON i.category_id = c.id AND i.is_deleted = FALSE
            WHERE c.id = $1
            GROUP BY c.id
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Category {id} not found")))
    }

    /// Get or create a category by name.  Returns the category ID.
    pub async fn get_or_create_category_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        name: &str,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO categories (name)
            VALUES ($1)
            ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
            RETURNING id
            "#,
        )
        .bind(name)
        .fetch_one(&mut **tx)
        .await?;
        Ok(id)
    }

    /// Create a new category.
    pub async fn create_category(&self, req: &CreateCategoryRequest) -> AppResult<Category> {
        // Validate parent exists for a friendlier error than a raw FK violation.
        if let Some(parent_id) = req.parent_category_id {
            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM categories WHERE id = $1)")
                .bind(parent_id)
                .fetch_one(&self.pool)
                .await?;
            if !exists {
                return Err(AppError::NotFound(format!("Parent category {parent_id} not found")));
            }
        }

        let row = sqlx::query_as::<_, Category>(
            r#"
            INSERT INTO categories (name, description, parent_category_id)
            VALUES ($1, $2, $3)
            RETURNING id, name, description, parent_category_id,
                      created_at, updated_at, 0::bigint AS item_count
            "#,
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.parent_category_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("uq_categories_name") {
                    return AppError::Conflict(format!("Category '{}' already exists", req.name));
                }
            }
            AppError::Database(e)
        })?;
        Ok(row)
    }

    /// Update a category name, description, or parent.
    /// Sending an empty description clears it to NULL.
    pub async fn update_category(&self, id: Uuid, req: &UpdateCategoryRequest) -> AppResult<Category> {
        // Pre-check: reject self-referencing parent for a clear error message.
        // The DB trigger check_category_no_cycle() also enforces cycle prevention.
        if let Some(Some(parent_id)) = req.parent_category_id {
            if parent_id == id {
                return Err(AppError::BadRequest("Category cannot be its own parent".into()));
            }
        }

        // Flatten Option<Option<Uuid>>: Some(x) means "update parent", None means "leave unchanged".
        let update_parent = req.parent_category_id.is_some();
        let parent_value: Option<Uuid> = req.parent_category_id.flatten();

        let row = sqlx::query_as::<_, Category>(
            r#"
            UPDATE categories
            SET name               = COALESCE($1, name),
                description        = CASE WHEN $2::bool THEN $3 ELSE description END,
                parent_category_id = CASE WHEN $4::bool THEN $5 ELSE parent_category_id END
            WHERE id = $6
            RETURNING id, name, description, parent_category_id,
                      created_at, updated_at,
                      (SELECT COUNT(*) FROM items WHERE category_id = $6 AND is_deleted = FALSE) AS item_count
            "#,
        )
        .bind(req.name.as_deref())
        .bind(req.description.is_some())  // $2: whether to update description
        .bind(req.description.as_deref().filter(|s| !s.is_empty())) // $3: empty → NULL
        .bind(update_parent)  // $4: whether to update parent
        .bind(parent_value)   // $5: NULL clears parent, UUID sets it
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("uq_categories_name") {
                    return AppError::Conflict(format!(
                        "Category '{}' already exists",
                        req.name.as_deref().unwrap_or("")
                    ));
                }
            }
            AppError::Database(e)
        })?
        .ok_or_else(|| AppError::NotFound(format!("Category {id} not found")))?;
        Ok(row)
    }

    /// Delete a category.  Items that reference it have their category_id set to NULL
    /// (ON DELETE SET NULL on items.category_id FK).
    pub async fn delete_category(&self, id: Uuid) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM categories WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Category {id} not found")));
        }
        Ok(())
    }
}
