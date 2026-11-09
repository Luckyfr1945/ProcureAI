use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SearchQuery {
    pub id: i64,
    pub job_id: i64,
    pub query_text: String,
    pub provider: String,
    pub page_number: i64,
    pub result_count: i64,
    pub exhausted: i64,
    pub created_at: String,
}
