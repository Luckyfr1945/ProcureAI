use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use procure_ai::api::app_router;
use procure_ai::config::Config;
use procure_ai::database::init_pool;
use procure_ai::state::AppState;
use procure_ai::workers::DiscoveryWorker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,procure_ai=debug".into()),
        )
        .init();

    let config = Config::from_env();
    tracing::info!("Starting ProcureAI in {} environment", config.app_env);

    // Initialize database pool and run migrations
    let db = init_pool(&config.database_url).await?;

    let state = AppState::new(config.clone(), db.clone());

    // Spawn background discovery worker
    let worker = Arc::new(DiscoveryWorker::new(state.config.clone(), db));
    tokio::spawn(async move {
        worker.run_loop().await;
    });

    let app = app_router(state);

    let bind_addr = format!("{}:{}", config.app_host, config.app_port);
    let listener = TcpListener::bind(&bind_addr).await?;
    tracing::info!("ProcureAI server listening on http://{}", bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
