use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SupplierSource {
    pub id: i64,
    pub supplier_id: i64,
    pub crawl_page_id: Option<i64>,
    pub source_url: String,
    pub source_type: Option<String>,
    pub evidence_json: Option<String>,
    pub extracted_at: String,
}
