use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<FieldError>,
}

#[derive(Debug, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Validation failed")]
    Validation(Vec<FieldError>),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Machine-readable error code for batch/structured error responses.
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Conflict(_) => "CONFLICT",
            AppError::Validation(_) => "VALIDATION_ERROR",
            AppError::Unauthorized => "UNAUTHORIZED",
            AppError::Forbidden => "FORBIDDEN",
            AppError::BadRequest(_) => "BAD_REQUEST",
            AppError::Database(_) => "INTERNAL_ERROR",
            AppError::Storage(_) => "STORAGE_ERROR",
            AppError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, details) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone(), vec![]),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone(), vec![]),
            AppError::Validation(fields) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR",
                "One or more fields failed validation".to_string(),
                fields.iter().map(|f| FieldError {
                    field: f.field.clone(),
                    message: f.message.clone(),
                }).collect(),
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Authentication required".to_string(),
                vec![],
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Insufficient permissions".to_string(),
                vec![],
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                msg.clone(),
                vec![],
            ),
            AppError::Database(e) => {
                tracing::error!(error = %e, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An internal error occurred".to_string(),
                    vec![],
                )
            }
            AppError::Storage(msg) => {
                tracing::error!(error = %msg, "Storage error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "STORAGE_ERROR",
                    "An internal storage error occurred".to_string(),
                    vec![],
                )
            }
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An internal error occurred".to_string(),
                    vec![],
                )
            }
        };

        let body = ErrorResponse {
            error: ErrorBody {
                code: code.to_string(),
                message,
                details,
            },
        };

        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    fn status_of(err: AppError) -> StatusCode {
        err.into_response().status()
    }

    #[test]
    fn not_found_returns_404() {
        assert_eq!(status_of(AppError::NotFound("x".into())), StatusCode::NOT_FOUND);
    }

    #[test]
    fn conflict_returns_409() {
        assert_eq!(status_of(AppError::Conflict("x".into())), StatusCode::CONFLICT);
    }

    #[test]
    fn validation_returns_422() {
        assert_eq!(
            status_of(AppError::Validation(vec![FieldError {
                field: "name".into(),
                message: "required".into(),
            }])),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    #[test]
    fn unauthorized_returns_401() {
        assert_eq!(status_of(AppError::Unauthorized), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn forbidden_returns_403() {
        assert_eq!(status_of(AppError::Forbidden), StatusCode::FORBIDDEN);
    }

    #[test]
    fn bad_request_returns_400() {
        assert_eq!(status_of(AppError::BadRequest("x".into())), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn storage_returns_500() {
        assert_eq!(status_of(AppError::Storage("x".into())), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn internal_returns_500() {
        assert_eq!(status_of(AppError::Internal("x".into())), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn error_code_not_found() {
        assert_eq!(AppError::NotFound("x".into()).error_code(), "NOT_FOUND");
    }

    #[test]
    fn error_code_conflict() {
        assert_eq!(AppError::Conflict("x".into()).error_code(), "CONFLICT");
    }

    #[test]
    fn error_code_bad_request() {
        assert_eq!(AppError::BadRequest("x".into()).error_code(), "BAD_REQUEST");
    }

    #[test]
    fn error_code_internal() {
        assert_eq!(AppError::Internal("x".into()).error_code(), "INTERNAL_ERROR");
    }

    #[test]
    fn error_code_unauthorized() {
        assert_eq!(AppError::Unauthorized.error_code(), "UNAUTHORIZED");
    }

    #[test]
    fn error_code_forbidden() {
        assert_eq!(AppError::Forbidden.error_code(), "FORBIDDEN");
    }
}
