use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SearchJob {
    pub id: String,
    pub user_id: i64,
    pub input_text: String,
    pub requirement_json: Option<String>,
    pub status: String,
    pub current_stage: Option<String>,
    pub round_count: i64,
    pub new_supplier_count: i64,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SearchQuery {
    pub id: i64,
    pub job_id: String,
    pub query_text: String,
    pub query_type: Option<String>,
    pub language: Option<String>,
    pub provider: Option<String>,
    pub page_or_offset: i64,
    pub result_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SearchJobEvent {
    pub id: i64,
    pub job_id: String,
    pub stage: String,
    pub message: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct JobSupplierResult {
    pub id: i64,
    pub job_id: String,
    pub supplier_id: i64,
    pub product_fit_score: f64,
    pub evidence_completeness_score: f64,
    pub rank_position: Option<i64>,
    pub created_at: String,
}
