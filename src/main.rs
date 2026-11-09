use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use tokio::net::TcpListener;

async fn health() -> Json<Value> {
    Json(json!({
        "application": "ProcureAI",
        "status": "running",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("APP_PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_addr = format!("{}:{}", host, port);

    let app = Router::new().route("/api/health", get(health));
    let listener = TcpListener::bind(&bind_addr).await?;

    tracing::info!("ProcureAI listening on http://{}", bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
