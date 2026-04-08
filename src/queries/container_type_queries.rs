use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::container_type::{ContainerType, CreateContainerTypeRequest, UpdateContainerTypeRequest};

/// Read/write query handler for container types.
#[derive(Clone)]
pub struct ContainerTypeQueries {
    pool: PgPool,
}

impl ContainerTypeQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// List all container types (ordered alphabetically by name).
    pub async fn list_all(&self) -> AppResult<Vec<ContainerType>> {
        let rows = sqlx::query_as::<_, ContainerType>(
            r#"
            SELECT id, name, description,
                   default_max_capacity_cc, default_max_weight_grams,
                   default_dimensions, default_location_schema, icon,
                   purpose, created_by, created_at, updated_at
            FROM container_types
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get a single container type by ID.
    pub async fn get_by_id(&self, id: Uuid) -> AppResult<ContainerType> {
        sqlx::query_as::<_, ContainerType>(
            r#"
            SELECT id, name, description,
                   default_max_capacity_cc, default_max_weight_grams,
                   default_dimensions, default_location_schema, icon,
                   purpose, created_by, created_at, updated_at
            FROM container_types
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("ContainerType {id} not found")))
    }

    /// Create a new container type.
    pub async fn create(&self, req: &CreateContainerTypeRequest, created_by: Uuid) -> AppResult<ContainerType> {
        let row = sqlx::query_as::<_, ContainerType>(
            r#"
            INSERT INTO container_types (
                name, description,
                default_max_capacity_cc, default_max_weight_grams,
                default_dimensions, default_location_schema, icon,
                purpose, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, name, description,
                      default_max_capacity_cc, default_max_weight_grams,
                      default_dimensions, default_location_schema, icon,
                      purpose, created_by, created_at, updated_at
            "#,
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.default_max_capacity_cc)
        .bind(req.default_max_weight_grams)
        .bind(&req.default_dimensions)
        .bind(&req.default_location_schema)
        .bind(&req.icon)
        .bind(req.purpose.as_deref().filter(|s| !s.is_empty())) // $8
        .bind(created_by)                                        // $9
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("container_types_name_created_by_key") {
                    return AppError::Conflict(format!(
                        "Container type '{}' already exists",
                        req.name
                    ));
                }
            }
            AppError::Database(e)
        })?;
        Ok(row)
    }

    /// Partially update a container type in a single query.
    /// Sending an empty string for description or icon clears it to NULL.
    pub async fn update(&self, id: Uuid, req: &UpdateContainerTypeRequest) -> AppResult<ContainerType> {
        let row = sqlx::query_as::<_, ContainerType>(
            r#"
            UPDATE container_types
            SET name                     = COALESCE($1, name),
                description              = CASE WHEN $2::bool THEN $3 ELSE description END,
                default_max_capacity_cc  = CASE WHEN $4::bool THEN $5 ELSE default_max_capacity_cc END,
                default_max_weight_grams = CASE WHEN $6::bool THEN $7 ELSE default_max_weight_grams END,
                default_dimensions       = CASE WHEN $8::bool THEN $9 ELSE default_dimensions END,
                default_location_schema  = CASE WHEN $10::bool THEN $11 ELSE default_location_schema END,
                icon                     = CASE WHEN $12::bool THEN $13 ELSE icon END,
                purpose                  = CASE WHEN $14::bool THEN $15 ELSE purpose END
            WHERE id = $16
            RETURNING id, name, description,
                      default_max_capacity_cc, default_max_weight_grams,
                      default_dimensions, default_location_schema, icon,
                      purpose, created_by, created_at, updated_at
            "#,
        )
        .bind(req.name.as_deref().filter(|s| !s.is_empty()))               // $1 — filter empty string so COALESCE keeps existing name
        .bind(req.description.is_some())                                   // $2
        .bind(req.description.as_deref().filter(|s| !s.is_empty()))        // $3
        .bind(req.default_max_capacity_cc.is_some())                       // $4
        .bind(req.default_max_capacity_cc)                                 // $5
        .bind(req.default_max_weight_grams.is_some())                      // $6
        .bind(req.default_max_weight_grams)                                // $7
        .bind(req.default_dimensions.is_some())                            // $8
        .bind(&req.default_dimensions)                                     // $9
        .bind(req.default_location_schema.is_some())                       // $10
        .bind(&req.default_location_schema)                                // $11
        .bind(req.icon.is_some())                                          // $12
        .bind(req.icon.as_deref().filter(|s| !s.is_empty()))               // $13
        .bind(req.purpose.is_some())                                       // $14
        .bind(req.purpose.as_deref().filter(|s| !s.is_empty()))            // $15
        .bind(id)                                                          // $16
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("container_types_name_created_by_key") {
                    return AppError::Conflict(format!(
                        "Container type '{}' already exists",
                        req.name.as_deref().unwrap_or("")
                    ));
                }
            }
            AppError::Database(e)
        })?
        .ok_or_else(|| AppError::NotFound(format!("ContainerType {id} not found")))?;
        Ok(row)
    }

    /// Delete a container type.  Containers referencing it have their
    /// container_type_id set to NULL by the FK ON DELETE SET NULL clause.
    pub async fn delete(&self, id: Uuid) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM container_types WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("ContainerType {id} not found")));
        }
        Ok(())
    }
}
