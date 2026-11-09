use crate::state::AppState;
use axum::{Json, extract::State};
use serde_json::{Value, json};

pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let db_status = match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => "connected",
        Err(_) => "error",
    };

    Json(json!({
        "application": "ProcureAI",
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
        "database": db_status,
        "environment": state.config.app_env
    }))
}
