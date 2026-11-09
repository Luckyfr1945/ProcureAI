use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use crate::auth::AuthenticatedUser;
use crate::database::repositories::{searches, suppliers};
use crate::state::AppState;

pub async fn get_dashboard_stats(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let total_suppliers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM suppliers")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let verified_suppliers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM suppliers WHERE verification_status IN ('verified', 'approved')",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let total_jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM search_jobs")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let running_jobs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM search_jobs WHERE status IN ('queued', 'running', 'stopping')",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let recent_jobs = searches::list_recent_jobs(&state.db, 5).await.unwrap_or_default();
    let recent_suppliers = suppliers::list_suppliers(&state.db, 5, 0, None, None).await.unwrap_or_default();

    Ok(Json(json!({
        "stats": {
            "total_suppliers": total_suppliers,
            "verified_suppliers": verified_suppliers,
            "total_jobs": total_jobs,
            "running_jobs": running_jobs
        },
        "recent_jobs": recent_jobs,
        "recent_suppliers": recent_suppliers
    })))
}

