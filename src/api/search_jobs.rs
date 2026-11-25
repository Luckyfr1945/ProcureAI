use crate::auth::AuthenticatedUser;
use crate::database::repositories::{evidence, products, search_jobs, suppliers};
use crate::domain::{CreateSearchJobRequest, ProductRequirement};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;

pub async fn create_search_job(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(dto): Json<CreateSearchJobRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if dto.input_text.len() > 8000
        || dto.target_regions.len() > 20
        || dto.required_attributes.len() > 50
        || dto.preferred_languages.len() > 5
        || dto.target_regions.iter().any(|s| s.len() > 200)
        || dto.required_attributes.iter().any(|s| s.len() > 500)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error":{"message":"Kebutuhan terlalu panjang. Batasi deskripsi 8000 karakter dan spesifikasi 50 syarat."}}),
            ),
        ));
    }
    if dto.input_text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "VALIDATION_ERROR",
                    "message": "input_text is required"
                }
            })),
        ));
    }

    let job_id = search_jobs::create_job(&state.db, user.user_id, dto.input_text.trim())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to create search job: {}", e)
                    }
                })),
            )
        })?;

    let pr_number = dto
        .pr_number
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // If user passed optional preferences, pre-seed pending requirement_json
    if !dto.target_regions.is_empty()
        || !dto.preferred_languages.is_empty()
        || !dto.required_attributes.is_empty()
        || pr_number.is_some()
    {
        let pending_req = ProductRequirement {
            product_name: dto.input_text.trim().to_string(),
            category: "__PENDING__".to_string(),
            brand: None,
            part_number: None,
            synonyms: Vec::new(),
            required_attributes: dto.required_attributes.clone(),
            optional_attributes: Vec::new(),
            supplier_types: vec![
                "manufacturer".to_string(),
                "authorized_distributor".to_string(),
                "distributor".to_string(),
                "wholesaler".to_string(),
            ],
            target_regions: if dto.target_regions.is_empty() {
                vec!["Indonesia".to_string()]
            } else {
                dto.target_regions.clone()
            },
            search_languages: if dto.preferred_languages.is_empty() {
                vec!["id".to_string(), "en".to_string()]
            } else {
                dto.preferred_languages.clone()
            },
            pr_number,
        };
        if let Ok(req_json) = serde_json::to_string(&pending_req) {
            let _ = search_jobs::set_job_requirement(&state.db, &job_id, &req_json).await;
        }
    }

    tracing::info!("Created search job {} for user {}", job_id, user.username);

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "job_id": job_id,
            "status": "queued",
            "current_stage": "understand",
            "message": "Deep research search job queued"
        })),
    ))
}

pub async fn list_search_jobs(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let jobs = search_jobs::list_user_jobs(&state.db, user.user_id)
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

pub async fn get_search_job(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    authorize_job(&state, &user, &job_id).await?;
    let job = search_jobs::get_job(&state.db, &job_id)
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

    authorize_job(&state, &user, &job_id).await?;
    let queries = search_jobs::get_queries(&state.db, &job_id)
        .await
        .unwrap_or_default();

    let requirement: Option<ProductRequirement> = job
        .requirement_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .filter(|r: &ProductRequirement| r.category != "__PENDING__");

    Ok(Json(json!({
        "job_id": job.id,
        "input_text": job.input_text,
        "status": job.status,
        "current_stage": job.current_stage,
        "round_count": job.round_count,
        "new_supplier_count": job.new_supplier_count,
        "query_count": queries.len(),
        "requirement": requirement,
        "error_message": job.error_message,
        "started_at": job.started_at,
        "finished_at": job.finished_at,
        "created_at": job.created_at
    })))
}

pub async fn stop_search_job(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    authorize_job(&state, &user, &job_id).await?;
    let job = search_jobs::get_job(&state.db, &job_id)
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

    match job {
        Some(j) if j.status == "queued" || j.status == "running" => {
            let _ = search_jobs::update_job_status(&state.db, &job_id, "stopping", None).await;
            let _ = search_jobs::record_event(
                &state.db,
                &job_id,
                j.current_stage.as_deref().unwrap_or("stopping"),
                Some("Stop requested by purchasing user"),
                None,
            )
            .await;

            Ok(Json(json!({
                "job_id": job_id,
                "status": "stopping",
                "message": "Stop signal dispatched to discovery worker"
            })))
        }
        Some(j) => Ok(Json(json!({
            "job_id": job_id,
            "status": j.status,
            "message": "Job is already stopped or finished"
        }))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "NOT_FOUND",
                    "message": "Search job not found"
                }
            })),
        )),
    }
}

pub async fn get_search_job_events(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    authorize_job(&state, &user, &job_id).await?;
    let events = search_jobs::get_job_events(&state.db, &job_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to fetch job events: {}", e)
                    }
                })),
            )
        })?;

    Ok(Json(events))
}

pub async fn get_search_job_queries(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    authorize_job(&state, &user, &job_id).await?;
    let queries = search_jobs::get_queries(&state.db, &job_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to fetch queries: {}", e)
                    }
                })),
            )
        })?;

    Ok(Json(queries))
}

pub async fn get_search_job_results(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    authorize_job(&state, &user, &job_id).await?;
    let detailed = search_jobs::get_job_results_detailed(&state.db, &job_id)
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

    let mut enriched = Vec::new();
    for item in detailed {
        let ev_items = evidence::get_evidence_by_supplier(&state.db, item.supplier.id)
            .await
            .unwrap_or_default();
        let sources = suppliers::get_supplier_sources(&state.db, item.supplier.id)
            .await
            .unwrap_or_default();

        // Also enrich offers with canonical product details
        let mut enriched_offers = Vec::new();
        for off in item.offers {
            let product = products::get_product_by_id(&state.db, off.product_id)
                .await
                .ok()
                .flatten();
            enriched_offers.push(json!({
                "offer": off,
                "product": product
            }));
        }

        enriched.push(json!({
            "result_id": item.result_id,
            "job_id": item.job_id,
            "rank_position": item.rank_position,
            "product_fit_score": item.product_fit_score,
            "evidence_completeness_score": item.evidence_completeness_score,
            "supplier": item.supplier,
            "offers": enriched_offers,
            "evidence_items": ev_items,
            "sources": sources
        }));
    }

    Ok(Json(enriched))
}

pub async fn export_search_job_xlsx(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    use axum::http::{HeaderMap, HeaderValue, header};
    use rust_xlsxwriter::{Format, Workbook};

    authorize_job(&state, &user, &job_id).await?;
    let detailed = search_jobs::get_job_results_detailed(&state.db, &job_id)
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

    let headers = [
        "Rank",
        "Product Fit (%)",
        "Evidence Completeness (%)",
        "Company Name",
        "Website",
        "Country",
        "Supplier Type",
        "Verification Status",
        "Public Email",
        "Public Phone",
        "Published Price",
        "Key Attributes (JSON)",
        "Source URL & Evidence Quote",
    ];

    for (col, text) in headers.iter().enumerate() {
        let _ = worksheet.write_string_with_format(0, col as u16, *text, &header_format);
    }

    for (idx, r) in detailed.iter().enumerate() {
        let row = (idx + 1) as u32;
        let _ = worksheet.write_number(row, 0, r.rank_position as f64);
        let _ = worksheet.write_number(row, 1, r.product_fit_score);
        let _ = worksheet.write_number(row, 2, r.evidence_completeness_score);
        let _ = worksheet.write_string(row, 3, &r.supplier.canonical_name);
        let _ = worksheet.write_string(row, 4, r.supplier.website.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(row, 5, r.supplier.country_name.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(
            row,
            6,
            r.supplier.supplier_type.as_deref().unwrap_or("unknown"),
        );
        let _ = worksheet.write_string(row, 7, &r.supplier.status);
        let _ = worksheet.write_string(row, 8, r.supplier.public_email.as_deref().unwrap_or(""));
        let _ = worksheet.write_string(row, 9, r.supplier.public_phone.as_deref().unwrap_or(""));

        let price_display = r
            .offers
            .iter()
            .find_map(|o| {
                o.price_amount.map(|amt| {
                    format!("{} {}", o.currency.as_deref().unwrap_or(""), amt)
                        .trim()
                        .to_string()
                })
            })
            .unwrap_or_else(|| "Tidak dipublikasikan / minta quotation".to_string());
        let _ = worksheet.write_string(row, 10, &price_display);

        let attrs_str = r
            .offers
            .first()
            .and_then(|o| o.attributes_json.as_deref())
            .unwrap_or("{}");
        let _ = worksheet.write_string(row, 11, attrs_str);

        let ev_items = evidence::get_evidence_by_supplier(&state.db, r.supplier.id)
            .await
            .unwrap_or_default();
        let sources = suppliers::get_supplier_sources(&state.db, r.supplier.id)
            .await
            .unwrap_or_default();

        let top_url = sources
            .first()
            .map(|s| s.canonical_url.as_str())
            .unwrap_or("");
        let top_quote = ev_items
            .iter()
            .map(|e| e.evidence_quote.as_str())
            .filter(|q| !q.is_empty())
            .take(2)
            .collect::<Vec<_>>()
            .join(" | ");
        let _ = worksheet.write_string(row, 12, &format!("{} [{}]", top_url, top_quote));
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
        "attachment; filename=\"procureai_job_{}_results.xlsx\"",
        job_id
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

pub async fn authorize_job(
    state: &AppState,
    user: &AuthenticatedUser,
    job: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let found = search_jobs::get_job(&state.db, job).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":{"message":"Database unavailable"}})),
        )
    })?;
    match found {
        Some(j) if j.user_id == user.user_id || user.role == "admin" => Ok(()),
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error":{"message":"Tidak memiliki akses ke pencarian ini"}})),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error":{"message":"Pencarian tidak ditemukan"}})),
        )),
    }
}
