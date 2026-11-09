use crate::auth::AuthenticatedUser;
use crate::database::repositories::searches;
use crate::models::{CreateSearchJobDto, SearchJobSummaryDto};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;

pub async fn create_search(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(dto): Json<CreateSearchJobDto>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if dto.request.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "VALIDATION_ERROR",
                    "message": "Search request is required"
                }
            })),
        ));
    }

    let options_json = dto
        .preferences
        .as_ref()
        .and_then(|p| serde_json::to_string(p).ok());

    let job_id = searches::create_job(
        &state.db,
        user.user_id,
        &dto.request,
        options_json.as_deref(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "DB_ERROR",
                    "message": format!("Failed to queue search job: {}", e)
                }
            })),
        )
    })?;

    // The background worker discovers queued jobs, or we can notify a channel/trigger worker
    tracing::info!("Created search job #{} for user {}", job_id, user.username);

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "job_id": job_id,
            "status": "queued",
            "message": "Supplier discovery job queued"
        })),
    ))
}

pub async fn get_search(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<Json<SearchJobSummaryDto>, (StatusCode, Json<serde_json::Value>)> {
    let job = searches::get_job(&state.db, id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "DB_ERROR",
                    "message": format!("Database error: {}", e)
                }
            })),
        )
    })?;

    let job = match job {
        Some(j) => j,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "code": "NOT_FOUND",
                        "message": "Search job not found"
                    }
                })),
            ));
        }
    };

    let suppliers_count = searches::count_job_suppliers(&state.db, id)
        .await
        .unwrap_or(0);

    let message = match job.status.as_str() {
        "queued" => "Waiting for worker to start".to_string(),
        "running" => format!(
            "Discovering and crawling sources (round {})",
            job.round_count
        ),
        "stopping" => "Stopping process as requested".to_string(),
        "stopped" => "Search process stopped by user".to_string(),
        "completed" => format!(
            "Search completed successfully with {} suppliers found",
            suppliers_count
        ),
        "failed" => job
            .error_message
            .clone()
            .unwrap_or_else(|| "Search job failed".to_string()),
        _ => "Status unknown".to_string(),
    };

    Ok(Json(SearchJobSummaryDto {
        job_id: job.id,
        status: job.status,
        round_count: job.round_count,
        discovered_urls: job.discovered_url_count,
        processed_urls: job.processed_url_count,
        suppliers_found: suppliers_count,
        message,
        stop_requested: job.stop_requested == 1,
        created_at: job.created_at,
    }))
}

pub async fn stop_search(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let stopped = searches::request_job_stop(&state.db, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Database error: {}", e)
                    }
                })),
            )
        })?;

    if stopped {
        Ok(Json(json!({
            "job_id": id,
            "status": "stopping",
            "message": "Stop signal sent to background search worker"
        })))
    } else {
        Ok(Json(json!({
            "job_id": id,
            "message": "Job is already finished or stopped"
        })))
    }
}

pub async fn get_search_results(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let results = searches::get_job_results(&state.db, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to get results: {}", e)
                    }
                })),
            )
        })?;

    Ok(Json(results))
}

pub async fn get_search_queries(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let queries = searches::get_queries(&state.db, id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "DB_ERROR",
                    "message": format!("Failed to get queries: {}", e)
                }
            })),
        )
    })?;

    Ok(Json(queries))
}

pub async fn get_search_pages(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pages = searches::get_crawl_pages(&state.db, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to get crawl pages: {}", e)
                    }
                })),
            )
        })?;

    Ok(Json(pages))
}

pub async fn export_search_results(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    use axum::http::{HeaderMap, HeaderValue, header};
    use rust_xlsxwriter::{Format, Workbook};

    let results = searches::get_job_results(&state.db, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to retrieve results: {}", e)
                    }
                })),
            )
        })?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_format = Format::new().set_bold();

    // Header row
    let headers = [
        "Rank",
        "Relevance Score",
        "Company Name",
        "Website",
        "Country",
        "City",
        "Email",
        "Phone",
        "Category",
        "Status",
        "Notes",
    ];

    for (col, text) in headers.iter().enumerate() {
        let _ = worksheet.write_string_with_format(0, col as u16, *text, &header_format);
    }

    // Data rows
    for (idx, r) in results.iter().enumerate() {
        let row = (idx + 1) as u32;
        let _ = worksheet.write_number(row, 0, (idx + 1) as f64);
        let _ = worksheet.write_number(row, 1, r.relevance_score);
        let _ = worksheet.write_string(row, 2, &r.supplier.company_name);
        let _ = worksheet.write_string(row, 3, r.supplier.website.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(row, 4, r.supplier.country.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(row, 5, r.supplier.city.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(row, 6, r.supplier.email.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(row, 7, r.supplier.phone.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(row, 8, r.supplier.category.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(row, 9, &r.supplier.verification_status);

        let notes_str = r
            .score_breakdown
            .as_ref()
            .map(|sb| sb.notes.join("; "))
            .unwrap_or_default();
        let _ = worksheet.write_string(row, 10, &notes_str);
    }

    let buffer = workbook.save_to_buffer().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "EXPORT_ERROR",
                    "message": format!("Failed to generate Excel file: {}", e)
                }
            })),
        )
    })?;

    let filename = format!(
        "attachment; filename=\"procure_ai_job_{}_results.xlsx\"",
        id
    );
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    if let Ok(cd) = HeaderValue::from_str(&filename) {
        resp_headers.insert(header::CONTENT_DISPOSITION, cd);
    }

    Ok((StatusCode::OK, resp_headers, buffer))
}

pub async fn list_recent_searches(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let jobs = searches::list_recent_jobs(&state.db, 20)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to list jobs: {}", e)
                    }
                })),
            )
        })?;

    Ok(Json(jobs))
}
