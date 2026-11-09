use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CrawlStatus {
    Discovered,
    Pending,
    Fetched,
    Failed,
    Skipped,
}

impl std::fmt::Display for CrawlStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discovered => write!(f, "discovered"),
            Self::Pending => write!(f, "pending"),
            Self::Fetched => write!(f, "fetched"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CrawlPage {
    pub id: i64,
    pub job_id: i64,
    pub url: String,
    pub normalized_url: String,
    pub domain: String,
    pub status: String,
    pub http_status: Option<i64>,
    pub page_title: Option<String>,
    pub content_hash: Option<String>,
    pub error_message: Option<String>,
    pub discovered_at: String,
    pub fetched_at: Option<String>,
}

