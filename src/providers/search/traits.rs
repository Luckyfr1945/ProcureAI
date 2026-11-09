use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchItem {
    pub url: String,
    pub title: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPage {
    pub query: String,
    pub page_number: u32,
    pub results: Vec<SearchItem>,
    pub has_next_page: bool,
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, page: u32) -> anyhow::Result<SearchPage>;
    fn provider_name(&self) -> &str;
}
