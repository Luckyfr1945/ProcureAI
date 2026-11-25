pub mod auth;
pub mod purchasing;
pub mod dashboard;
pub mod health;
pub mod search_jobs;
pub mod suppliers;

use crate::state::AppState;
use axum::Router;
use axum::routing::{get, patch, post};
use tower_http::services::ServeDir;

pub fn app_router(state: AppState) -> Router {
    let v1_routes = Router::new()
        // Health
        .route("/health", get(health::health_check))
        // Auth
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/users", post(auth::register_user))
        // Search Jobs (v2)
        .route("/search-jobs", post(search_jobs::create_search_job))
        .route("/search-jobs", get(search_jobs::list_search_jobs))
        .route("/search-jobs/{job_id}", get(search_jobs::get_search_job))
        .route("/search-jobs/{job_id}/stop", post(search_jobs::stop_search_job))
        .route("/search-jobs/{job_id}/events", get(search_jobs::get_search_job_events))
        .route("/search-jobs/{job_id}/queries", get(search_jobs::get_search_job_queries))
        .route("/search-jobs/{job_id}/results", get(search_jobs::get_search_job_results))
        .route("/search-jobs/{job_id}/export.xlsx", get(search_jobs::export_search_job_xlsx))
        .route("/search-jobs/{job_id}/purchasing", get(purchasing::list))
        .route("/search-jobs/{job_id}/purchasing/{supplier_id}", post(purchasing::save))
        // Suppliers
        .route("/suppliers", get(suppliers::list_suppliers))
        .route("/suppliers/{id}", get(suppliers::get_supplier))
        .route("/suppliers/{id}", patch(suppliers::update_supplier))
        .route("/suppliers/{id}/verify", post(suppliers::verify_supplier))
        .route("/suppliers/{id}/offers", get(suppliers::get_supplier_offers))
        // Dashboard
        .route("/dashboard", get(dashboard::get_dashboard_stats))
        .route("/dashboard/summary", get(dashboard::get_dashboard_stats))
        .route_layer(axum::middleware::from_fn(protect_origin));

    Router::new()
        .nest("/api/v1", v1_routes)
        // Convenience top-level aliases for health & auth if probed directly
        .route("/api/health", get(health::health_check))
        .fallback_service(ServeDir::new("frontend"))
        .with_state(state)
}

async fn protect_origin(request: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !matches!(*request.method(), axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS) {
        if let Some(origin) = request.headers().get(axum::http::header::ORIGIN) {
            let valid = origin.to_str().ok().and_then(|s| url::Url::parse(s).ok()).is_some_and(|origin| {
                request.headers().get(axum::http::header::HOST).and_then(|h| h.to_str().ok())
                    .and_then(|host| url::Url::parse(&format!("{}://{}", origin.scheme(), host)).ok())
                    .is_some_and(|host| origin.origin() == host.origin() && ["http", "https"].contains(&origin.scheme()))
            });
            if !valid { return (axum::http::StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error":{"message":"Origin permintaan tidak diizinkan"}}))).into_response(); }
        }
    }
    next.run(request).await
}
