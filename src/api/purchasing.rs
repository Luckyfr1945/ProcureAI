use crate::{auth::AuthenticatedUser, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

type ApiError = (StatusCode, Json<Value>);
fn error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({"error": {"message": message}})))
}
fn db_error(e: sqlx::Error) -> ApiError {
    tracing::error!("Purchasing database error: {}", e);
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Gagal menyimpan atau memuat data purchasing",
    )
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Review {
    pub version: Option<i64>,
    pub status: String,
    pub notes: String,
    pub pic: String,
    pub follow_up_date: String,
    pub quotation_ref: String,
    pub item: String,
    pub quantity: Option<f64>,
    pub unit: String,
    pub currency: String,
    pub unit_price: Option<f64>,
    pub tax_amount: Option<f64>,
    pub shipping_cost: Option<f64>,
    pub moq: String,
    pub lead_time: String,
    pub payment_terms: String,
    pub valid_until: String,
}
impl Review {
    fn validate(&self) -> Result<(), ApiError> {
        if !["shortlisted", "contacted", "quoted", "selected", "rejected"]
            .contains(&self.status.as_str())
        {
            return Err(error(StatusCode::BAD_REQUEST, "Status seleksi tidak valid"));
        }
        for text in [
            &self.notes,
            &self.pic,
            &self.quotation_ref,
            &self.item,
            &self.unit,
            &self.moq,
            &self.lead_time,
            &self.payment_terms,
        ] {
            if text.len() > 4000 {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "Teks maksimal 4000 byte per kolom",
                ));
            }
        }
        for value in [
            self.unit_price,
            self.tax_amount,
            self.shipping_cost,
            self.quantity,
        ]
        .into_iter()
        .flatten()
        {
            if !value.is_finite() || value < 0.0 || value > 1e15 {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "Angka harus positif dan dalam batas yang wajar",
                ));
            }
        }
        if self.quantity.is_some_and(|n| n <= 0.0) {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "Jumlah harus lebih dari nol",
            ));
        }
        if !self.currency.is_empty()
            && (self.currency.len() != 3 || !self.currency.bytes().all(|c| c.is_ascii_uppercase()))
        {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "Gunakan kode mata uang tiga huruf, misalnya IDR",
            ));
        }
        for date in [&self.follow_up_date, &self.valid_until] {
            if !date.is_empty()
                && (date.len() != 10
                    || chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err())
            {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "Tanggal harus berformat YYYY-MM-DD",
                ));
            }
        }
        if self.unit_price.is_some()
            && (self.quantity.is_none()
                || self.unit.trim().is_empty()
                || self.currency.is_empty()
                || self.item.trim().is_empty())
        {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "Lengkapi barang, jumlah, satuan, dan mata uang quotation",
            ));
        }
        if self.status == "quoted" && self.unit_price.is_none() {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "Isi harga untuk status penawaran diterima",
            ));
        }
        if ["selected", "rejected"].contains(&self.status.as_str()) && self.notes.trim().is_empty()
        {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "Isi alasan pemilihan atau penolakan supplier",
            ));
        }
        Ok(())
    }
}
async fn authorize(state: &AppState, user: &AuthenticatedUser, job: &str) -> Result<(), ApiError> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM search_jobs WHERE id = ?")
        .bind(job)
        .fetch_optional(&state.db)
        .await
        .map_err(db_error)?;
    match owner {
        None => Err(error(StatusCode::NOT_FOUND, "Pencarian tidak ditemukan")),
        Some(id) if id == user.user_id || user.role == "admin" => Ok(()),
        _ => Err(error(
            StatusCode::FORBIDDEN,
            "Anda tidak memiliki akses ke seleksi ini",
        )),
    }
}
pub async fn list(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(job): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&state, &user, &job).await?;
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as("SELECT r.supplier_id, json_set(r.data_json, '$.version', r.version), r.updated_at, u.username FROM purchasing_reviews r JOIN users u ON u.id = r.updated_by WHERE r.job_id = ? ORDER BY r.updated_at DESC, r.supplier_id")
        .bind(&job).fetch_all(&state.db).await.map_err(db_error)?;
    let history: Vec<(i64, String, String, String)> = sqlx::query_as("SELECT h.supplier_id, h.data_json, h.created_at, u.username FROM purchasing_review_history h JOIN users u ON u.id = h.changed_by WHERE h.job_id = ? ORDER BY h.id DESC LIMIT 100")
        .bind(&job).fetch_all(&state.db).await.map_err(db_error)?;
    let pack = |rows: Vec<(i64, String, String, String)>| {
        rows.into_iter().map(|(id, data, date, user)| json!({"supplier_id": id, "review": serde_json::from_str::<Value>(&data).unwrap_or(Value::Null), "at": date, "by": user})).collect::<Vec<_>>()
    };
    Ok(Json(
        json!({"reviews": pack(rows), "history": pack(history)}),
    ))
}
pub async fn save(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((job, supplier)): Path<(String, i64)>,
    Json(mut review): Json<Review>,
) -> Result<Json<Value>, ApiError> {
    authorize(&state, &user, &job).await?;
    review.validate()?;
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM job_supplier_results WHERE job_id = ? AND supplier_id = ?",
    )
    .bind(&job)
    .bind(supplier)
    .fetch_one(&state.db)
    .await
    .map_err(db_error)?;
    if exists == 0 {
        return Err(error(
            StatusCode::NOT_FOUND,
            "Supplier bukan kandidat pada pencarian ini",
        ));
    }
    let expected_version = review.version.unwrap_or(0);
    if expected_version < 0 {
        return Err(error(StatusCode::BAD_REQUEST, "Versi tidak valid"));
    }
    review.version = Some(expected_version + 1);
    let data = serde_json::to_string(&review)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "Data tidak valid"))?;
    let mut tx = state.db.begin().await.map_err(db_error)?;
    let saved = sqlx::query("INSERT INTO purchasing_reviews (job_id,supplier_id,data_json,updated_by,version) SELECT ?,?,?,?,1 WHERE ?=0 OR EXISTS(SELECT 1 FROM purchasing_reviews WHERE job_id=? AND supplier_id=?) ON CONFLICT(job_id,supplier_id) DO UPDATE SET data_json=excluded.data_json, updated_by=excluded.updated_by, updated_at=CURRENT_TIMESTAMP, version=purchasing_reviews.version+1 WHERE purchasing_reviews.version=?")
        .bind(&job).bind(supplier).bind(&data).bind(user.user_id).bind(expected_version).bind(&job).bind(supplier).bind(expected_version)
        .execute(&mut *tx).await.map_err(db_error)?;
    if saved.rows_affected() != 1 {
        return Err(error(
            StatusCode::CONFLICT,
            "Catatan telah berubah. Muat ulang lalu periksa perubahan sebelum menyimpan kembali.",
        ));
    }
    sqlx::query("INSERT INTO purchasing_review_history (job_id,supplier_id,data_json,changed_by) VALUES (?,?,?,?)")
        .bind(&job).bind(supplier).bind(&data).bind(user.user_id).execute(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(Json(
        json!({"saved": true, "version": expected_version + 1}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_quotation_and_decision() {
        let mut r = Review {
            status: "quoted".into(),
            ..Default::default()
        };
        assert!(r.validate().is_err());
        r.unit_price = Some(100.0);
        assert!(r.validate().is_err());
        r.quantity = Some(2.0);
        r.unit = "pcs".into();
        r.currency = "IDR".into();
        r.item = "Bearing".into();
        assert!(r.validate().is_ok());
        r.shipping_cost = Some(-1.0);
        assert!(r.validate().is_err());
        r.shipping_cost = None;
        r.status = "selected".into();
        assert!(r.validate().is_err());
        r.notes = "Spesifikasi dan waktu kirim sesuai".into();
        assert!(r.validate().is_ok());
        r.valid_until = "2026-02-30".into();
        assert!(r.validate().is_err());
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::{
        config::Config,
        database::repositories::{search_jobs, users},
        models::UserRole,
    };

    #[tokio::test]
    async fn saves_per_job_with_history_and_checks_access() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let owner = users::create_user(&pool, "buyer", "hash", UserRole::Purchasing)
            .await
            .unwrap();
        let other = users::create_user(&pool, "other", "hash", UserRole::Purchasing)
            .await
            .unwrap();
        let job = search_jobs::create_job(&pool, owner, "Bearing")
            .await
            .unwrap();
        let second_job = search_jobs::create_job(&pool, owner, "Valve")
            .await
            .unwrap();
        let supplier = sqlx::query(
            "INSERT INTO suppliers(canonical_name, normalized_name) VALUES('Vendor','vendor')",
        )
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();
        search_jobs::save_job_result(&pool, &job, supplier, 80.0, 60.0, 1)
            .await
            .unwrap();
        let state = AppState::new(Config::from_env(), pool.clone());
        let buyer = AuthenticatedUser {
            user_id: owner,
            username: "buyer".into(),
            role: "purchasing".into(),
        };
        let outsider = AuthenticatedUser {
            user_id: other,
            username: "other".into(),
            role: "purchasing".into(),
        };
        assert_eq!(
            list(State(state.clone()), outsider.clone(), Path(job.clone()))
                .await
                .unwrap_err()
                .0,
            StatusCode::FORBIDDEN
        );
        let review = || Review {
            status: "shortlisted".into(),
            notes: "Perlu cek stok".into(),
            ..Default::default()
        };
        assert_eq!(
            save(
                State(state.clone()),
                outsider,
                Path((job.clone(), supplier)),
                Json(review())
            )
            .await
            .unwrap_err()
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            save(
                State(state.clone()),
                buyer.clone(),
                Path((second_job.clone(), supplier)),
                Json(review())
            )
            .await
            .unwrap_err()
            .0,
            StatusCode::NOT_FOUND
        );
        for version in 0..2 {
            let mut next = review();
            next.version = Some(version);
            let _ = save(
                State(state.clone()),
                buyer.clone(),
                Path((job.clone(), supplier)),
                Json(next),
            )
            .await
            .unwrap();
        }
        assert_eq!(
            save(
                State(state.clone()),
                buyer.clone(),
                Path((job.clone(), supplier)),
                Json(review())
            )
            .await
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
        let Json(data) = list(State(state.clone()), buyer.clone(), Path(job))
            .await
            .unwrap();
        assert_eq!(data["reviews"].as_array().unwrap().len(), 1);
        assert_eq!(data["history"].as_array().unwrap().len(), 2);
        assert_eq!(data["reviews"][0]["review"]["notes"], "Perlu cek stok");
        let Json(empty) = list(State(state), buyer, Path(second_job)).await.unwrap();
        assert!(empty["reviews"].as_array().unwrap().is_empty());
        let status: String = sqlx::query_scalar("SELECT status FROM suppliers WHERE id=?")
            .bind(supplier)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "discovered");
    }
}
