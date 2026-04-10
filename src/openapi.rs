//! OpenAPI specification and Swagger UI setup.

use utoipa::OpenApi;

use crate::api::system_routes;
use crate::errors;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Homorg API",
        version = "1.0.0",
        description = "Event-sourced personal inventory management system",
        license(name = "MIT")
    ),
    servers(
        (url = "/api/v1", description = "API v1")
    ),
    paths(
        system_routes::health,
        system_routes::health_live,
        system_routes::health_ready,
    ),
    components(schemas(
        errors::ErrorResponse,
        errors::ErrorBody,
        errors::FieldError,
        system_routes::HealthResponse,
        system_routes::ReadinessResponse,
        system_routes::DatabaseHealth,
    )),
    tags(
        (name = "system", description = "Health checks and system status"),
    )
)]
pub struct ApiDoc;
