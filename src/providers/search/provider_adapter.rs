use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use crate::config::Config;
use crate::providers::search::traits::{SearchItem, SearchPage, SearchProvider};

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

        let q_lower = query.to_lowercase();

        // 1. Check if user is searching for plastic bottles, caps, packaging
        let results = if q_lower.contains("botol")
            || q_lower.contains("plastik")
            || q_lower.contains("cap")
            || q_lower.contains("bottle")
            || q_lower.contains("tutup")
            || q_lower.contains("packaging")
        {
            if page == 1 {
                vec![
                    SearchItem {
                        url: "https://www.berkatplastindo.co.id/tutup-botol-plastik".to_string(),
                        title: "PT Berkat Plastindo Utama - Pabrik Tutup Botol Plastik PP & HDPE".to_string(),
                        snippet: "Produsen dan distributor aneka tutup botol plastik, fliptop, ulir galon, dan kemasan kosmetik/farmasi di Indonesia.".to_string(),
                    },
                    SearchItem {
                        url: "https://www.nusantaraplast.id/products/caps-closure".to_string(),
                        title: "Nusantara Plastic Caps & Closures - Supplier Resmi Tutup Botol".to_string(),
                        snippet: "Spesialis tutup botol tamper-evident, tutup botol air mineral, dan custom injection molding berskala industri.".to_string(),
                    },
                    SearchItem {
                        url: "https://www.globalcaps.com/en/products".to_string(),
                        title: "Global Cap & Packaging Solutions Ltd - International Supplier".to_string(),
                        snippet: "Worldwide manufacturer of plastic closures, screw caps, and industrial dispensing caps with international shipping.".to_string(),
                    },
                ]
            } else if page == 2 {
                vec![
                    SearchItem {
                        url: "https://www.indopackaging.com/katalog-tutup-botol".to_string(),
                        title: "Indo Packaging Solusindo - Grosir Tutup Botol dan Kemasan Plastik".to_string(),
                        snippet: "Menyediakan tutup botol plastik berbagai ukuran (28mm, 30mm, 38mm) siap kirim seluruh Indonesia.".to_string(),
                    },
                    SearchItem {
                        url: "https://www.captop-industry.com/wholesale".to_string(),
                        title: "CapTop Industries - High Volume Bottle Cap Exporter".to_string(),
                        snippet: "Wholesale food-grade plastic bottle caps, oil bottle caps, and custom color closures for beverage manufacturers.".to_string(),
                    },
                ]
            } else {
                vec![
                    SearchItem {
                        url: "https://www.mitraplastindo.co.id/kontak-sales".to_string(),
                        title: "Mitra Plastindo Jaya - Distributor Komponen Plastik Pengemasan".to_string(),
                        snippet: "Katalog lengkap tutup botol, pump botol, dan tutup segel keamanan dengan harga distributor pabrik.".to_string(),
                    },
                ]
            }
        } else if q_lower.contains("bearing")
            || q_lower.contains("skf")
            || q_lower.contains("timken")
            || q_lower.contains("fag")
            || q_lower.contains("nsk")
        {
            // 2. Industrial bearings
            if page == 1 {
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
                        title: format!("BearingNet Industrial Inventory Exchange - {}", q_lower),
                        snippet: "B2B portal connecting global bearing distributors and manufacturer representatives.".to_string(),
                    },
                ]
            }
        } else {
            // 3. Generic query fallback matching user's keywords
            let clean_slug: String = q_lower
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == ' ')
                .collect::<String>()
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join("-");

            let capitalized: String = q_lower
                .split_whitespace()
                .take(3)
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            if page == 1 {
                vec![
                    SearchItem {
                        url: format!("https://www.indo-{}.co.id/products", clean_slug),
                        title: format!("PT {} Utama - Pabrik & Distributor Resmi", capitalized),
                        snippet: format!("Produsen dan supplier terkemuka untuk kebutuhan {} dengan standar kualitas industri dan pengiriman nasional.", query),
                    },
                    SearchItem {
                        url: format!("https://www.global-{}.com/catalog", clean_slug),
                        title: format!("Global {} Manufacturing & Export Ltd", capitalized),
                        snippet: format!("Worldwide verified manufacturer and exporter of {}, supporting wholesale procurement.", query),
                    },
                    SearchItem {
                        url: format!("https://www.mitra-{}.com/supplier", clean_slug),
                        title: format!("Mitra {} Indonesia - Pusat Pengadaan Perusahaan", capitalized),
                        snippet: format!("Solusi pengadaan B2B untuk {}, melayani tender dan suplai kontrak rutin pabrik.", query),
                    },
                ]
            } else if page == 2 {
                vec![
                    SearchItem {
                        url: format!("https://www.{}-industry.com/wholesale", clean_slug),
                        title: format!("{} Industrial Wholesale & Distribution", capitalized),
                        snippet: format!("Direct distributor with bulk pricing and specification datasheets for {}.", query),
                    },
                ]
            } else {
                vec![
                    SearchItem {
                        url: format!("https://www.{}-supply.net/portal", clean_slug),
                        title: format!("{} B2B Supply Network", capitalized),
                        snippet: format!("B2B marketplace network connecting certified suppliers of {}.", query),
                    },
                ]
            }
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
        let p_name = self.provider_name.to_lowercase();

        let mut req = self.client.get(&self.base_url);

        // Customize query params by provider format
        if p_name.contains("serpapi") {
            let start = (page.saturating_sub(1)) * 10;
            req = req.query(&[
                ("q", query),
                ("api_key", &self.api_key),
                ("start", &start.to_string()),
                ("engine", &"google".to_string()),
            ]);
        } else if p_name.contains("brave") {
            let offset = (page.saturating_sub(1)) * 10;
            let mut headers = HeaderMap::new();
            if let Ok(val) = HeaderValue::from_str(&self.api_key) {
                headers.insert("X-Subscription-Token", val);
            }
            req = req.headers(headers).query(&[
                ("q", query),
                ("offset", &offset.to_string()),
            ]);
        } else if p_name.contains("searxng") {
            req = req.query(&[
                ("q", query),
                ("format", &"json".to_string()),
                ("pageno", &page.to_string()),
            ]);
        } else {
            // Generic external search API
            req = req.query(&[
                ("q", query),
                ("page", &page.to_string()),
                ("api_key", &self.api_key),
            ]);
        }

        let resp = req.send().await?;

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

        let mut results = Vec::new();

        // Check SerpApi / Google format
        let items_opt = resp_json
            .get("organic_results")
            .or_else(|| resp_json.get("results"))
            .or_else(|| resp_json.get("items"))
            .or_else(|| {
                resp_json
                    .get("web")
                    .and_then(|w| w.get("results"))
            });

        if let Some(items) = items_opt {
            if let Some(arr) = items.as_array() {
                for item in arr {
                    let url = item
                        .get("link")
                        .or_else(|| item.get("url"))
                        .and_then(|v| v.as_str());
                    let title = item.get("title").and_then(|v| v.as_str()).unwrap_or_default();
                    let snippet = item
                        .get("snippet")
                        .or_else(|| item.get("description"))
                        .or_else(|| item.get("content"))
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
        "http" | "serpapi" | "searxng" | "brave" | "google" => {
            if config.search_api_base_url.is_empty() {
                tracing::warn!("Search API base URL is empty, falling back to dynamic mock search provider");
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
