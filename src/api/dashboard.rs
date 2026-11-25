use crate::auth::AuthenticatedUser;
use crate::database::repositories::{products, search_jobs, suppliers};
use crate::state::AppState;
use axum::{Json, extract::State, http::StatusCode};
use serde_json::json;

pub async fn get_dashboard_stats(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let total_suppliers = suppliers::count_suppliers(&state.db)
        .await
        .map_err(dashboard_error)?;
    let verified_suppliers = suppliers::count_verified_suppliers(&state.db)
        .await
        .map_err(dashboard_error)?;
    let published_prices = products::count_published_prices(&state.db)
        .await
        .map_err(dashboard_error)?;

    let total_jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM search_jobs WHERE user_id=?")
        .bind(user.user_id)
        .fetch_one(&state.db)
        .await
        .map_err(dashboard_error)?;

    let running_jobs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM search_jobs WHERE user_id=? AND status IN ('queued', 'running', 'stopping')",
    )
    .bind(user.user_id)
    .fetch_one(&state.db)
    .await
    .map_err(dashboard_error)?;

    let recent_jobs = search_jobs::list_user_jobs(&state.db, user.user_id)
        .await
        .map_err(dashboard_error)?
        .into_iter()
        .take(5)
        .collect::<Vec<_>>();

    let recent_suppliers = suppliers::list_suppliers(&state.db, 5, 0, None, None)
        .await
        .map_err(dashboard_error)?;

    Ok(Json(json!({
        "stats": {
            "total_suppliers": total_suppliers,
            "verified_suppliers": verified_suppliers,
            "published_prices": published_prices,
            "total_jobs": total_jobs,
            "running_jobs": running_jobs
        },
        "recent_jobs": recent_jobs,
        "recent_suppliers": recent_suppliers
    })))
}

fn dashboard_error(e: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    tracing::error!("Dashboard failed: {}", e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error":{"message":"Gagal memuat ringkasan. Coba muat ulang."}})),
    )
}
