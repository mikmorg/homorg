//! Shared test harness for integration tests.
//!
//! Spins up a pgvector/pgvector:pg16 container via testcontainers,
//! creates extensions, runs migrations, and returns a fully-wired `AppState`.

use std::sync::Arc;

use homorg::config::AppConfig;
use homorg::events::store::EventStore;
use homorg::storage::{LocalStorage, StorageBackend};
use homorg::AppState;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use testcontainers::core::IntoContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use uuid::Uuid;

/// Everything returned from setup — keep the container alive for the test's lifetime.
pub struct TestContext {
    pub state: Arc<AppState>,
    pub admin_id: Uuid,
    /// Hold this to keep the Postgres container running; dropped at test end.
    pub _container: ContainerAsync<GenericImage>,
    pub _tmpdir: tempfile::TempDir,
}

/// Spin up an isolated Postgres container, run migrations, seed data, and
/// return a fully-wired `AppState`.
pub async fn setup() -> TestContext {
    // Start pgvector/pgvector:pg16 container
    // NOTE: with_exposed_port / with_wait_for must be called on GenericImage
    // BEFORE the ImageExt methods (with_env_var) which convert to ContainerRequest.
    let container: ContainerAsync<GenericImage> = GenericImage::new("pgvector/pgvector", "pg16")
        .with_exposed_port(5432.tcp())
        .with_wait_for(testcontainers::core::WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_USER", "test")
        .with_env_var("POSTGRES_PASSWORD", "test")
        .with_env_var("POSTGRES_DB", "homorg_test")
        .start()
        .await
        .expect("Failed to start Postgres container");

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get mapped port");

    let database_url = format!("postgres://test:test@127.0.0.1:{host_port}/homorg_test");

    // Connect with retry — container may need a moment
    let pool = connect_with_retry(&database_url, 10).await;

    // Create required extensions (normally done by docker init script)
    sqlx::query("CREATE EXTENSION IF NOT EXISTS ltree")
        .execute(&pool)
        .await
        .expect("Failed to create ltree extension");
    sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_trgm")
        .execute(&pool)
        .await
        .expect("Failed to create pg_trgm extension");
    sqlx::query("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"")
        .execute(&pool)
        .await
        .expect("Failed to create uuid-ossp extension");
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool)
        .await
        .expect("Failed to create vector extension");

    // Run all migrations
    homorg::db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    // Build config
    let config = test_config(&database_url);

    // Temp dir for storage
    let tmpdir = tempfile::tempdir().expect("Failed to create temp dir");
    let storage: Arc<dyn StorageBackend> = Arc::new(LocalStorage::new(tmpdir.path().to_str().unwrap()));

    // Build AppState
    let event_store = EventStore::new(pool.clone());
    let state = Arc::new(AppState::new(config, pool, event_store, storage));

    // Seed an admin user
    let admin_id = seed_admin_user(&state).await;

    TestContext {
        state,
        admin_id,
        _container: container,
        _tmpdir: tmpdir,
    }
}

/// Retry connection — the container may take a second to accept connections.
async fn connect_with_retry(url: &str, max_attempts: u32) -> PgPool {
    for attempt in 1..=max_attempts {
        match PgPoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .connect(url)
            .await
        {
            Ok(pool) => return pool,
            Err(e) => {
                if attempt == max_attempts {
                    panic!("Failed to connect to test database after {max_attempts} attempts: {e}");
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
    unreachable!()
}

/// Build a minimal AppConfig for testing.
fn test_config(database_url: &str) -> AppConfig {
    AppConfig {
        database_url: database_url.to_string(),
        jwt_secret: "test-jwt-secret-that-is-at-least-32-characters-long".to_string(),
        jwt_access_ttl_secs: 3600,
        jwt_refresh_ttl_days: 30,
        listen_addr: "127.0.0.1:0".to_string(),
        barcode_prefix: "HOM".to_string(),
        barcode_pad_width: 6,
        storage_path: "./data/test-images".to_string(),
        max_batch_size: 500,
        cors_origins: vec!["*".to_string()],
        db_max_connections: 5,
        db_min_connections: 1,
        db_acquire_timeout_secs: 30,
        db_idle_timeout_secs: 600,
        db_max_lifetime_secs: 1800,
        max_upload_bytes: 10_485_760,
        allowed_image_mimes: vec!["image/jpeg".into(), "image/png".into(), "image/webp".into()],
        rate_limit_enabled: false,
        rate_limit_rps: 100,
        rate_limit_burst: 200,
        log_format: "text".to_string(),
    }
}

/// Create a test admin user with a known password hash, linked to a personal container.
async fn seed_admin_user(state: &AppState) -> Uuid {
    let admin_id = Uuid::new_v4();
    // Pre-computed Argon2id hash for "testpassword123"
    let password_hash = homorg::auth::password::hash_password("testpassword123")
        .await
        .expect("Failed to hash password");

    state
        .user_queries
        .create(admin_id, "testadmin", &password_hash, Some("Test Admin"), "admin")
        .await
        .expect("Failed to create admin user");

    // Create the admin's personal container under the Users container
    let container_id = Uuid::new_v4();
    let barcode = state
        .barcode_commands
        .generate_barcode()
        .await
        .expect("Failed to generate barcode");

    let create_req = homorg::models::item::CreateItemRequest {
        system_barcode: Some(barcode.barcode),
        parent_id: homorg::constants::USERS_ID,
        name: Some("testadmin's Container".to_string()),
        description: None,
        category: None,
        tags: None,
        is_container: Some(true),
        coordinate: None,
        location_schema: None,
        max_capacity_cc: None,
        max_weight_grams: None,
        container_type_id: None,
        dimensions: None,
        weight_grams: None,
        is_fungible: None,
        fungible_quantity: None,
        fungible_unit: None,
        external_codes: None,
        condition: None,
        currency: None,
        acquisition_date: None,
        acquisition_cost: None,
        current_value: None,
        depreciation_rate: None,
        warranty_expiry: None,
        metadata: None,
    };

    let metadata = homorg::models::event::EventMetadata::default();
    state
        .item_commands
        .create_item(container_id, &create_req, admin_id, &metadata)
        .await
        .expect("Failed to create admin container");

    state
        .user_queries
        .set_container(admin_id, container_id)
        .await
        .expect("Failed to link admin to container");

    admin_id
}

/// Helper: build a CreateItemRequest with sensible defaults.
#[allow(dead_code)]
pub fn make_item_request(
    barcode: &str,
    parent_id: Uuid,
    name: &str,
    is_container: bool,
) -> homorg::models::item::CreateItemRequest {
    homorg::models::item::CreateItemRequest {
        system_barcode: Some(barcode.to_string()),
        parent_id,
        name: Some(name.to_string()),
        description: None,
        category: None,
        tags: None,
        is_container: Some(is_container),
        coordinate: None,
        location_schema: None,
        max_capacity_cc: None,
        max_weight_grams: None,
        container_type_id: None,
        dimensions: None,
        weight_grams: None,
        is_fungible: None,
        fungible_quantity: None,
        fungible_unit: None,
        external_codes: None,
        condition: None,
        currency: None,
        acquisition_date: None,
        acquisition_cost: None,
        current_value: None,
        depreciation_rate: None,
        warranty_expiry: None,
        metadata: None,
    }
}
