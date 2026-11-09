use crate::config::Config;
use crate::providers::search::traits::{SearchItem, SearchPage, SearchProvider};
use async_trait::async_trait;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

pub struct MockSearchProvider;

#[async_trait]
impl SearchProvider for MockSearchProvider {
    async fn search(&self, query: &str, page: u32) -> anyhow::Result<SearchPage> {
        let max_pages = 3;
        if page > max_pages {
            return Ok(SearchPage {
                query: query.to_string(),
                page_number: page,
                results: Vec::new(),
                has_next_page: false,
            });
        }

        let clean_q = query.to_lowercase();
        let results = if page == 1 {
            vec![
                SearchItem {
                    url: "https://www.skf.com/group/products/rolling-bearings".to_string(),
                    title: format!("SKF Official - Industrial Bearings for {}", query),
                    snippet: "Global manufacturer of high performance deep groove ball bearings and roller solutions.".to_string(),
                },
                SearchItem {
                    url: "https://www.timken.com/products/timken-engineered-bearings/".to_string(),
                    title: format!("The Timken Company - Precision Bearings ({})", query),
                    snippet: "Engineered bearings, industrial motion products and mechanical power transmissions.".to_string(),
                },
                SearchItem {
                    url: "https://www.nsk.com/products/bearings/".to_string(),
                    title: format!("NSK Global Motion & Control - Bearing Catalog for {}", query),
                    snippet: "NSK produces high-precision ball bearings and linear motion guides worldwide.".to_string(),
                },
            ]
        } else if page == 2 {
            vec![
                SearchItem {
                    url: "https://www.schaeffler.de/en/products-and-solutions/industrial/".to_string(),
                    title: format!("Schaeffler Group - INA & FAG Bearing Range - {}", query),
                    snippet: "High-precision rolling and plain bearings for industrial machinery applications.".to_string(),
                },
                SearchItem {
                    url: "https://www.koyobearings.com/catalog".to_string(),
                    title: format!("JTEKT Koyo Bearings - Distributor & Wholesale Catalog - {}", query),
                    snippet: "Authorized industrial supplier with worldwide export and bulk procurement options.".to_string(),
                },
            ]
        } else {
            vec![
                SearchItem {
                    url: "https://www.bearingnet.net/inventory".to_string(),
                    title: format!("BearingNet Industrial Inventory Exchange - {}", clean_q),
                    snippet: "B2B portal connecting global bearing distributors and manufacturer representatives.".to_string(),
                },
            ]
        };

        Ok(SearchPage {
            query: query.to_string(),
            page_number: page,
            results,
            has_next_page: page < max_pages,
        })
    }

    fn provider_name(&self) -> &str {
        "mock"
    }
}

pub struct HttpSearchProvider {
    client: Client,
    base_url: String,
    api_key: String,
    provider_name: String,
}

impl HttpSearchProvider {
    pub fn new(base_url: String, api_key: String, provider_name: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP search client");

        Self {
            client,
            base_url,
            api_key,
            provider_name,
        }
    }
}

#[async_trait]
impl SearchProvider for HttpSearchProvider {
    async fn search(&self, query: &str, page: u32) -> anyhow::Result<SearchPage> {
        let resp = self
            .client
            .get(&self.base_url)
            .query(&[
                ("q", query),
                ("page", &page.to_string()),
                ("api_key", &self.api_key),
            ])
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(anyhow::anyhow!("Search API rate limit reached (HTTP 429)"));
        }

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "Search API returned HTTP error: {}",
                resp.status()
            ));
        }

        let resp_json: serde_json::Value = resp.json().await?;

        // Standardized parsing: works with standard search engine JSON responses
        let mut results = Vec::new();
        if let Some(items) = resp_json
            .get("results")
            .or_else(|| resp_json.get("organic_results"))
        {
            if let Some(arr) = items.as_array() {
                for item in arr {
                    let url = item
                        .get("url")
                        .or_else(|| item.get("link"))
                        .and_then(|v| v.as_str());
                    let title = item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let snippet = item
                        .get("snippet")
                        .or_else(|| item.get("description"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();

                    if let Some(u) = url {
                        results.push(SearchItem {
                            url: u.to_string(),
                            title: title.to_string(),
                            snippet: snippet.to_string(),
                        });
                    }
                }
            }
        }

        let has_next_page = !results.is_empty();

        Ok(SearchPage {
            query: query.to_string(),
            page_number: page,
            results,
            has_next_page,
        })
    }

    fn provider_name(&self) -> &str {
        &self.provider_name
    }
}

pub fn create_search_provider(config: &Config) -> Arc<dyn SearchProvider> {
    match config.search_provider.to_lowercase().as_str() {
        "http" | "serpapi" | "searxng" => {
            if config.search_api_base_url.is_empty() {
                tracing::warn!(
                    "Search API base URL is empty, falling back to mock search provider"
                );
                Arc::new(MockSearchProvider)
            } else {
                Arc::new(HttpSearchProvider::new(
                    config.search_api_base_url.clone(),
                    config.search_api_key.clone(),
                    config.search_provider.clone(),
                ))
            }
        }
        _ => Arc::new(MockSearchProvider),
    }
}
