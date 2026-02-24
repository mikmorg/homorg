pub mod api;
pub mod auth;
pub mod commands;
pub mod config;
pub mod constants;
pub mod db;
pub mod errors;
pub mod events;
pub mod models;
pub mod queries;
pub mod storage;

use std::sync::Arc;

use commands::barcode_commands::BarcodeCommands;
use commands::item_commands::ItemCommands;
use commands::undo_commands::UndoCommands;
use config::AppConfig;
use events::store::EventStore;
use queries::container_queries::ContainerQueries;
use queries::item_queries::ItemQueries;
use queries::search_queries::SearchQueries;
use sqlx::PgPool;
use storage::StorageBackend;

/// Shared application state, injected into all Axum handlers via `State<Arc<AppState>>`.
pub struct AppState {
    pub config: AppConfig,
    pub pool: PgPool,
    pub event_store: EventStore,
    pub item_commands: ItemCommands,
    pub undo_commands: UndoCommands,
    pub barcode_commands: BarcodeCommands,
    pub item_queries: ItemQueries,
    pub container_queries: ContainerQueries,
    pub search_queries: SearchQueries,
    pub storage: Arc<dyn StorageBackend>,
}
