use crate::models::supplier::Supplier;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SearchResult {
    pub id: i64,
    pub job_id: i64,
    pub supplier_id: i64,
    pub relevance_score: f64,
    pub score_breakdown_json: Option<String>,
    pub first_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub product_match_score: f64,
    pub location_score: f64,
    pub company_profile_score: f64,
    pub contact_score: f64,
    pub price_score: f64,
    pub total_score: f64,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct EnrichedSearchResultDto {
    pub result_id: i64,
    pub job_id: i64,
    pub relevance_score: f64,
    pub score_breakdown: Option<ScoreBreakdown>,
    pub supplier: Supplier,
    pub first_seen_at: String,
}
