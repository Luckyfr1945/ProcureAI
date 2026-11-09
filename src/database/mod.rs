use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
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
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Run migrations automatically
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;
    tracing::info!("Database migrations applied successfully.");

    Ok(pool)
}

