use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EvidenceItem {
    pub id: i64,
    pub supplier_id: Option<i64>,
    pub offer_id: Option<i64>,
    pub field_path: String,
    pub field_value_json: Option<String>,
    pub source_id: i64,
    pub evidence_quote: String,
    pub confidence: f64,
    pub verification_status: String,
    pub verified_by_user_id: Option<i64>,
    pub verified_at: Option<String>,
}
