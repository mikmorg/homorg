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

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use commands::barcode_commands::BarcodeCommands;
use commands::item_commands::ItemCommands;
use commands::undo_commands::UndoCommands;
use config::AppConfig;
use events::store::EventStore;
use queries::container_queries::ContainerQueries;
use queries::container_type_queries::ContainerTypeQueries;
use queries::item_queries::ItemQueries;
use queries::search_queries::SearchQueries;
use queries::session_queries::SessionRepository;
use queries::stats_queries::StatsQueries;
use queries::taxonomy_queries::TaxonomyQueries;
use queries::token_queries::TokenRepository;
use queries::user_queries::UserQueries;
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
    pub user_queries: UserQueries,
    pub token_repository: TokenRepository,
    pub session_repository: SessionRepository,
    pub stats_queries: StatsQueries,
    pub container_type_queries: ContainerTypeQueries,
    pub taxonomy_queries: TaxonomyQueries,
    pub storage: Arc<dyn StorageBackend>,
    /// API-5: Tracks whether a projection rebuild is currently running.
    pub rebuild_in_progress: Arc<AtomicBool>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: AppConfig,
        pool: PgPool,
        event_store: EventStore,
        storage: Arc<dyn StorageBackend>,
    ) -> Self {
        let item_commands = ItemCommands::new(pool.clone(), event_store.clone());
        let session_repository = SessionRepository::new(pool.clone());
        let undo_commands = UndoCommands::new(pool.clone(), event_store.clone(), session_repository.clone());
        let barcode_commands = BarcodeCommands::new(pool.clone(), config.clone(), event_store.clone());
        let item_queries = ItemQueries::new(pool.clone());
        let container_queries = ContainerQueries::new(pool.clone());
        let search_queries = SearchQueries::new(pool.clone());
        let user_queries = UserQueries::new(pool.clone());
        let token_repository = TokenRepository::new(pool.clone());
        let stats_queries = StatsQueries::new(pool.clone());
        let container_type_queries = ContainerTypeQueries::new(pool.clone());
        let taxonomy_queries = TaxonomyQueries::new(pool.clone());

        Self {
            config,
            pool,
            event_store,
            item_commands,
            undo_commands,
            barcode_commands,
            item_queries,
            container_queries,
            search_queries,
            user_queries,
            token_repository,
            session_repository,
            stats_queries,
            container_type_queries,
            taxonomy_queries,
            storage,
            rebuild_in_progress: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Guard that clears the `rebuild_in_progress` flag when dropped.
pub struct RebuildGuard(pub Arc<AtomicBool>);

impl Drop for RebuildGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

pub use std::sync::atomic::Ordering as AtomicOrdering;