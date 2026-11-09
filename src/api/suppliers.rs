use crate::auth::AuthenticatedUser;
use crate::database::repositories::suppliers;
use crate::models::{Supplier, SupplierProduct, SupplierSource, UpdateSupplierDto};
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
}

pub async fn list_suppliers(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Query(params): Query<SupplierListQuery>,
) -> Result<Json<Vec<Supplier>>, (StatusCode, Json<serde_json::Value>)> {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).max(0);

    let list = suppliers::list_suppliers(
        &state.db,
        limit,
        offset,
        params.country.as_deref(),
        params.status.as_deref(),
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
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<Json<Supplier>, (StatusCode, Json<serde_json::Value>)> {
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

    match supplier {
        Some(s) => Ok(Json(s)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "NOT_FOUND",
                    "message": "Supplier not found"
                }
            })),
        )),
    }
}

pub async fn update_supplier(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(dto): Json<UpdateSupplierDto>,
) -> Result<Json<Supplier>, (StatusCode, Json<serde_json::Value>)> {
    let updated = suppliers::update_supplier(&state.db, id, dto)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "DB_ERROR",
                        "message": format!("Failed to update supplier: {}", e)
                    }
                })),
            )
        })?;

    match updated {
        Some(s) => Ok(Json(s)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "NOT_FOUND",
                    "message": "Supplier not found"
                }
            })),
        )),
    }
}

pub async fn get_supplier_sources(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<Json<Vec<SupplierSource>>, (StatusCode, Json<serde_json::Value>)> {
    let sources = suppliers::get_sources(&state.db, id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "DB_ERROR",
                    "message": format!("Failed to get supplier sources: {}", e)
                }
            })),
        )
    })?;

    Ok(Json(sources))
}

pub async fn get_supplier_products(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<Json<Vec<SupplierProduct>>, (StatusCode, Json<serde_json::Value>)> {
    let products = suppliers::get_products(&state.db, id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "DB_ERROR",
                    "message": format!("Failed to get supplier products: {}", e)
                }
            })),
        )
    })?;

    Ok(Json(products))
}
