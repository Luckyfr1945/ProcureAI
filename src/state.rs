use crate::config::Config;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: SqlitePool,
    pub login_attempts: Arc<tokio::sync::Mutex<std::collections::HashMap<String, (u32, std::time::Instant)>>>,
    pub login_slots: Arc<tokio::sync::Semaphore>,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool) -> Self {
        Self {
            config: Arc::new(config),
            db,
            login_attempts: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
}
