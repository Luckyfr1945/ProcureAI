use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Stopping,
    Stopped,
    Completed,
    Failed,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SearchJob {
    pub id: i64,
    pub user_id: i64,
    pub original_request: String,
    pub status: String,
    pub options_json: Option<String>,
    pub stop_requested: i64,
    pub round_count: i64,
    pub discovered_url_count: i64,
    pub processed_url_count: i64,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPreferences {
    #[serde(default)]
    pub countries: Vec<String>,
    #[serde(default)]
    pub exclude_marketplaces: bool,
    #[serde(default)]
    pub prioritize_manufacturers: bool,
}

impl Default for SearchPreferences {
    fn default() -> Self {
        Self {
            countries: Vec::new(),
            exclude_marketplaces: false,
            prioritize_manufacturers: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSearchJobDto {
    pub request: String,
    #[serde(default)]
    pub preferences: Option<SearchPreferences>,
}

#[derive(Debug, Serialize)]
pub struct SearchJobSummaryDto {
    pub job_id: i64,
    pub status: String,
    pub round_count: i64,
    pub discovered_urls: i64,
    pub processed_urls: i64,
    pub suppliers_found: i64,
    pub message: String,
    pub stop_requested: bool,
    pub created_at: String,
}
