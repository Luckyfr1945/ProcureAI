pub mod auth;
pub mod dashboard;
pub mod health;
pub mod searches;
pub mod suppliers;

use crate::state::AppState;
use axum::Router;
use axum::routing::{get, patch, post};
use tower_http::services::ServeDir;

pub fn app_router(state: AppState) -> Router {
    let api_routes = Router::new()
        // Health
        .route("/api/health", get(health::health_check))
        // Auth
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/users", post(auth::register_user))
        // Searches
        .route("/api/search", post(searches::create_search))
        .route("/api/search", get(searches::list_recent_searches))
        .route("/api/search/{id}", get(searches::get_search))
        .route("/api/search/{id}/stop", post(searches::stop_search))
        .route(
            "/api/search/{id}/results",
            get(searches::get_search_results),
        )
        .route(
            "/api/search/{id}/queries",
            get(searches::get_search_queries),
        )
        .route("/api/search/{id}/pages", get(searches::get_search_pages))
        .route(
            "/api/search/{id}/export",
            get(searches::export_search_results),
        )
        // Suppliers
        .route("/api/suppliers", get(suppliers::list_suppliers))
        .route("/api/suppliers/{id}", get(suppliers::get_supplier))
        .route("/api/suppliers/{id}", patch(suppliers::update_supplier))
        .route(
            "/api/suppliers/{id}/sources",
            get(suppliers::get_supplier_sources),
        )
        .route(
            "/api/suppliers/{id}/products",
            get(suppliers::get_supplier_products),
        )
        // Dashboard
        .route("/api/dashboard", get(dashboard::get_dashboard_stats));

    Router::new()
        .merge(api_routes)
        .fallback_service(ServeDir::new("frontend"))
        .with_state(state)
}
