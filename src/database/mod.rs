use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

pub mod repositories;

pub async fn init_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    // If the database path has a parent directory, ensure it exists
    if let Some(path_str) = database_url.strip_prefix("sqlite://") {
        let clean_path = path_str.split('?').next().unwrap_or(path_str);
        if let Some(parent) = Path::new(clean_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
    }

    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Run migrations automatically
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations applied successfully.");

    // Seed initial admin if empty
    seed_initial_admin_if_needed(&pool).await?;

    Ok(pool)
}

pub async fn seed_initial_admin_if_needed(pool: &SqlitePool) -> anyhow::Result<()> {
    let count = repositories::users::count_users(pool).await?;
    if count == 0 {
        let admin_password = std::env::var("DEFAULT_ADMIN_PASSWORD")
            .unwrap_or_else(|_| "AdminProcureAI2026!".to_string());
        let hash = crate::auth::password::hash_password(&admin_password)?;
        let id =
            repositories::users::create_user(pool, "admin", &hash, crate::models::UserRole::Admin)
                .await?;
        tracing::info!("Initialized default administrator user 'admin' (id={})", id);
    }
    Ok(())
}
