use crate::auth::AuthenticatedUser;
use crate::database::repositories::{evidence, products, suppliers};
use crate::domain::{Supplier, SupplierOffer};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct SupplierListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub country: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SupplierDetailQuery {
    pub job_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSupplierPayload {
    pub canonical_name: Option<String>,
    pub public_email: Option<String>,
    pub public_phone: Option<String>,
    pub supplier_type: Option<String>,
    pub address: Option<String>,
    pub status: Option<String>,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VerifySupplierPayload {
    pub evidence_ids: Option<Vec<i64>>,
    pub verification_status: Option<String>, // e.g. verified_by_purchasing or rejected
}

pub async fn list_suppliers(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Query(params): Query<SupplierListQuery>,
) -> Result<Json<Vec<Supplier>>, (StatusCode, Json<serde_json::Value>)> {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).max(0);

    if params.q.as_ref().is_some_and(|q| q.len() > 500) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":{"message":"Kata pencarian terlalu panjang"}})),
        ));
    }
    if let Some(q) = params.q.as_deref() {
        let list = sqlx::query_as::<_, Supplier>("SELECT * FROM suppliers WHERE (?='' OR instr(lower(canonical_name || ' ' || coalesce(normalized_domain,'') || ' ' || coalesce(country_name,'') || ' ' || coalesce(business_category,'') || ' ' || coalesce(address,'')),lower(?)) > 0) AND (? IS NULL OR status=?) AND (? IS NULL OR country_code=? OR country_name=?) ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(q).bind(q).bind(params.status.as_deref()).bind(params.status.as_deref()).bind(params.country.as_deref()).bind(params.country.as_deref()).bind(params.country.as_deref()).bind(limit).bind(offset)
            .fetch_all(&state.db).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":{"message":"Gagal mencari supplier"}}))))?;
        return Ok(Json(list));
    }
    let list = suppliers::list_suppliers(
        &state.db,
        limit,
        offset,
        params.status.as_deref(),
        params.country.as_deref(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "DB_ERROR",
                    "message": format!("Failed to list suppliers: {}", e)
                }
            })),
        )
    })?;

    Ok(Json(list))
}

pub async fn get_supplier(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<i64>,
    Query(params): Query<SupplierDetailQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let supplier = suppliers::get_supplier_by_id(&state.db, id)
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

    let supplier = match supplier {
        Some(s) => s,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "code": "NOT_FOUND",
                        "message": "Supplier not found"
                    }
                })),
            ));
        }
    };

    let sources = suppliers::get_supplier_sources(&state.db, id)
        .await
        .unwrap_or_default();
    let ev_items = evidence::get_evidence_by_supplier(&state.db, id)
        .await
        .unwrap_or_default();
    let offers = if let Some(job_id) = params.job_id {
        super::search_jobs::authorize_job(&state, &user, &job_id).await?;
        crate::database::repositories::search_jobs::get_job_results_detailed(&state.db, &job_id)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error":{"message":"Gagal memuat produk pencarian"}})),
                )
            })?
            .into_iter()
            .find(|r| r.supplier.id == id)
            .map(|r| r.offers)
            .unwrap_or_default()
    } else {
        products::get_supplier_offers(&state.db, id)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error":{"message":"Gagal memuat produk"}})),
                )
            })?
    };

    let needs_review: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM supplier_offers WHERE supplier_id=? AND needs_review=1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":{"message":"Gagal memuat status data"}})),
        )
    })?;
    Ok(Json(json!({
        "offers_needing_review": needs_review,
        "supplier": supplier,
        "sources": sources,
        "evidence_items": ev_items,
        "offers": offers
    })))
}

pub async fn update_supplier(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(dto): Json<UpdateSupplierPayload>,
) -> Result<Json<Supplier>, (StatusCode, Json<serde_json::Value>)> {
    let bad = |message: &str| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":{"message":message}})),
        )
    };
    if dto
        .status
        .as_deref()
        .is_some_and(|status| !valid_status(status))
    {
        return Err(bad("Status supplier tidak valid"));
    }
    for value in [
        &dto.canonical_name,
        &dto.public_email,
        &dto.public_phone,
        &dto.address,
        &dto.country_code,
        &dto.country_name,
    ] {
        if value
            .as_ref()
            .is_some_and(|v| v.len() > 4000 || v.chars().any(|c| c.is_control() && c != '\n'))
        {
            return Err(bad("Data profil tidak valid"));
        }
    }
    if dto
        .canonical_name
        .as_deref()
        .is_some_and(|v| v.trim().is_empty())
    {
        return Err(bad("Nama perusahaan wajib diisi"));
    }
    if let Some(email) = dto.public_email.as_deref().filter(|v| !v.is_empty()) {
        if crate::utils::text::extract_valid_emails(email)
            .iter()
            .all(|e| e != &email.to_lowercase())
        {
            return Err(bad("Format email tidak valid"));
        }
    }
    let country = if dto.country_code.is_some() || dto.country_name.is_some() {
        Some(
            dto.country_code
                .as_deref()
                .and_then(crate::services::location::country_code_from_name)
                .or_else(|| {
                    dto.country_name
                        .as_deref()
                        .and_then(crate::services::location::country_code_from_name)
                })
                .ok_or_else(|| bad("Negara tidak dikenali"))?,
        )
    } else {
        None
    };
    let fail = |e: sqlx::Error| {
        tracing::error!("Profile update failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":{"message":"Gagal menyimpan koreksi supplier"}})),
        )
    };
    let mut tx = state.db.begin().await.map_err(fail)?;
    let result = sqlx::query("UPDATE suppliers SET canonical_name=coalesce(?,canonical_name), normalized_name=coalesce(?,normalized_name), public_email=CASE WHEN ? IS NULL THEN public_email ELSE nullif(?,'') END, public_phone=CASE WHEN ? IS NULL THEN public_phone ELSE nullif(?,'') END, address=CASE WHEN ? IS NULL THEN address ELSE nullif(?,'') END, supplier_type=coalesce(?,supplier_type), status=coalesce(?,status), country_code=coalesce(?,country_code), country_name=coalesce(?,country_name), updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(dto.canonical_name.as_deref().map(str::trim)).bind(dto.canonical_name.as_deref().map(crate::utils::text::normalize_company_name))
        .bind(&dto.public_email).bind(&dto.public_email).bind(&dto.public_phone).bind(&dto.public_phone).bind(&dto.address).bind(&dto.address)
        .bind(&dto.supplier_type).bind(&dto.status).bind(country).bind(country.map(crate::services::location::canonical_country_name)).bind(id)
        .execute(&mut *tx).await.map_err(fail)?;
    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error":{"message":"Supplier tidak ditemukan"}})),
        ));
    }
    if dto.canonical_name.is_some()
        || dto.address.is_some()
        || dto.public_email.is_some()
        || dto.public_phone.is_some()
        || country.is_some()
    {
        sqlx::query("INSERT INTO supplier_profile_corrections(supplier_id,corrected_by) VALUES (?,?) ON CONFLICT(supplier_id) DO UPDATE SET corrected_by=excluded.corrected_by, corrected_at=CURRENT_TIMESTAMP")
            .bind(id).bind(user.user_id).execute(&mut *tx).await.map_err(fail)?;
    }
    tx.commit().await.map_err(fail)?;
    let updated = suppliers::get_supplier_by_id(&state.db, id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":{"message":"Gagal memuat profil"}})),
            )
        })?;
    Ok(Json(updated.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error":{"message":"Supplier tidak ditemukan"}})),
        )
    })?))
}

pub async fn verify_supplier(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<i64>,
    payload: Option<Json<VerifySupplierPayload>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let target_status = payload
        .as_ref()
        .and_then(|p| p.verification_status.as_deref())
        .unwrap_or("verified_by_purchasing");

    let fail = |e: sqlx::Error| {
        tracing::error!("Verification failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":{"message":"Gagal menyimpan verifikasi"}})),
        )
    };
    if !valid_status(target_status) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":{"message":"Status supplier tidak valid"}})),
        ));
    }
    let mut tx = state.db.begin().await.map_err(fail)?;
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM suppliers WHERE id=?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(fail)?;
    if exists == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error":{"message":"Supplier tidak ditemukan"}})),
        ));
    }
    // Company validation never automatically validates every claim.
    if let Some(ids) = payload.as_ref().and_then(|p| p.evidence_ids.as_ref()) {
        for evidence_id in ids {
            let affected = sqlx::query("UPDATE evidence_items SET verification_status='verified', verified_by_user_id=?, verified_at=CURRENT_TIMESTAMP WHERE id=? AND supplier_id=?")
                .bind(user.user_id).bind(evidence_id).bind(id).execute(&mut *tx).await.map_err(fail)?.rows_affected();
            if affected != 1 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error":{"message":"Bukti bukan milik supplier ini atau tidak ditemukan"}}),
                    ),
                ));
            }
        }
    }
    sqlx::query("UPDATE suppliers SET status=?, updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(target_status)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(fail)?;
    tx.commit().await.map_err(fail)?;
    let supplier = suppliers::get_supplier_by_id(&state.db, id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":{"message":"Gagal memuat supplier"}})),
            )
        })?;

    Ok(Json(json!({
        "supplier_id": id,
        "status": target_status,
        "verified_by": user.username,
        "supplier": supplier
    })))
}

pub async fn get_supplier_offers(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<Json<Vec<SupplierOffer>>, (StatusCode, Json<serde_json::Value>)> {
    let offers = products::get_supplier_offers(&state.db, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to get supplier offers: {}", e)
                    }
                })),
            )
        })?;

    Ok(Json(offers))
}

fn valid_status(status: &str) -> bool {
    [
        "discovered",
        "verified_by_purchasing",
        "contacted",
        "approved",
        "rejected",
    ]
    .contains(&status)
}
