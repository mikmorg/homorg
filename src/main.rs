use std::sync::Arc;

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use homorg::api;
use homorg::commands::barcode_commands::BarcodeCommands;
use homorg::commands::item_commands::ItemCommands;
use homorg::commands::undo_commands::UndoCommands;
use homorg::config::AppConfig;
use homorg::db;
use homorg::events::store::EventStore;
use homorg::queries::container_queries::ContainerQueries;
use homorg::queries::item_queries::ItemQueries;
use homorg::queries::search_queries::SearchQueries;
use homorg::storage::LocalStorage;
use homorg::AppState;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("homorg=debug,info")),
        )
        .init();

    // Load configuration
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env().expect("Failed to load configuration");

    tracing::info!("Starting Homorg daemon on {}", config.listen_addr);

    // Create database pool and run migrations
    let pool = db::create_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");

    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Database migrations complete");

    // Initialize storage backend
    let storage = LocalStorage::new(&config.storage_path);
    storage.init().await.expect("Failed to initialize storage");
    let storage: Arc<dyn homorg::storage::StorageBackend> = Arc::new(storage);

    // Build service layers
    let event_store = EventStore::new(pool.clone());
    let item_commands = ItemCommands::new(pool.clone(), event_store.clone());
    let undo_commands = UndoCommands::new(pool.clone(), event_store.clone());
    let barcode_commands = BarcodeCommands::new(pool.clone(), config.clone());
    let item_queries = ItemQueries::new(pool.clone());
    let container_queries = ContainerQueries::new(pool.clone());
    let search_queries = SearchQueries::new(pool.clone());

    // Build shared state
    let state = Arc::new(AppState {
        config: config.clone(),
        pool: pool.clone(),
        event_store,
        item_commands,
        undo_commands,
        barcode_commands,
        item_queries,
        container_queries,
        search_queries,
        storage,
    });

    // Build CORS layer from config
    let cors = if config.cors_origins.len() == 1 && config.cors_origins[0] == "*" {
        CorsLayer::permissive()
    } else {
        let origins: Vec<_> = config
            .cors_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    // Build router
    let app = api::build_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    // Start server
    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("Homorg daemon listening on {}", config.listen_addr);

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
