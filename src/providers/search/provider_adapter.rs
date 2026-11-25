use async_trait::async_trait;
use base64::Engine;
use percent_encoding::percent_decode_str;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use crate::config::Config;
use crate::providers::search::traits::{SearchItem, SearchPage, SearchProvider};
use crate::utils::url_safety::extract_domain;

fn is_excluded_domain(domain: &str) -> bool {
    let d = domain.to_lowercase();
    d.contains("google.")
        || d.contains("bing.com")
        || d.contains("yahoo.com")
        || d.contains("brave.com")
        || d.contains("yimg.com")
        || d.contains("duckduckgo.com")
        || d.contains("youtube.com")
        || d.contains("bilibili.tv")
        || d.contains("tiktok.com")
        || d.contains("instagram.com")
        || d.contains("facebook.com")
        || d.contains("twitter.com")
        || d.contains("x.com")
        || d.contains("pinterest.com")
        || d.contains("wikipedia.org")
        || d.contains("wiktionary.org")
        || d.contains("cookpad.com")
        || d.contains("brainly.co.id")
        || d.contains("jobstreet.")
        || d.contains("glints.com")
        || d.contains("jooble.org")
        || d.contains("dealls.com")
        || d.contains("kompas.com")
        || d.contains("detik.com")
        || d.contains("pikiran-rakyat.com")
        || d.contains("bisnis.com")
        || d.contains("tempo.co")
        || d.contains("tribunnews.com")
        || d.ends_with(".go.id")
}

fn decode_yahoo_redirect(url_str: &str) -> String {
    if !url_str.contains("r.search.yahoo.com") {
        return url_str.to_string();
    }
    if let Some(pos) = url_str.find("/RU=") {
        let rest = &url_str[pos + 4..];
        if let Some(end) = rest.find('/') {
            let encoded = &rest[..end];
            let decoded = percent_decode_str(encoded).decode_utf8_lossy();
            if decoded.starts_with("http") {
                return decoded.to_string();
            }
        }
    }
    url_str.to_string()
}

fn decode_bing_redirect(url_str: &str) -> String {
    if let Some(pos) = url_str.find("&u=a1") {
        let encoded = url_str[pos + 5..].split('&').next().unwrap_or("");
        if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded) {
            if let Ok(decoded) = String::from_utf8(bytes) {
                if decoded.starts_with("http") {
                    return decoded;
                }
            }
        }
    }
    url_str.to_string()
}

fn clean_b2b_keywords(query: &str) -> Vec<String> {
    let stop = [
        "supplier", "distributor", "manufacturer", "wholesaler", "exporter",
        "pabrik", "produsen", "jual", "grosir", "resmi", "authorized",
        "factory", "indonesia", "surabaya", "jakarta", "bandung", "semarang",
        "tangerang", "bekasi", "cikarang", "medan", "pt", "cv", "b2b",
        "contact", "email", "whatsapp", "telephone", "price", "harga",
        "quote", "datasheet", "catalog", "specification", "pdf",
        "untuk", "buat", "dengan", "yang", "dan", "atau", "cari",
    ];
    query
        .split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 2 && !stop.contains(&w.as_str()))
        .take(3)
        .collect()
}

pub struct LiveSearchProvider {
    #[allow(dead_code)]
    client: Client,
    brave_cooldown_until: Mutex<Option<Instant>>,
    yahoo_cooldown_until: Mutex<Option<Instant>>,
}

impl LiveSearchProvider {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
            ),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("id-ID,id;q=0.9,en-US;q=0.8,en;q=0.7"),
        );

        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .default_headers(headers)
            .build()
            .expect("Failed to build live search client");

        Self {
            client,
            brave_cooldown_until: Mutex::new(None),
            yahoo_cooldown_until: Mutex::new(None),
        }
    }

    async fn fetch_via_curl(&self, url: &str) -> (u16, String) {
        let output = tokio::process::Command::new("curl")
            .arg("-sL")
            .arg("--max-time")
            .arg("10")
            .arg("-w")
            .arg("\n%{http_code}")
            .arg("-A")
            .arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .arg("-H")
            .arg("Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .arg("-H")
            .arg("Accept-Language: id-ID,id;q=0.9,en-US;q=0.8,en;q=0.7")
            .arg(url)
            .output()
            .await;

        let Ok(out) = output else {
            return (0, String::new());
        };
        let raw = String::from_utf8_lossy(&out.stdout);
        if let Some((body, code_str)) = raw.rsplit_once('\n') {
            let code = code_str.trim().parse::<u16>().unwrap_or(0);
            return (code, body.to_string());
        }
        (0, String::new())
    }

    async fn search_yahoo(&self, query: &str, page: u32) -> Vec<SearchItem> {
        {
            let cd = self.yahoo_cooldown_until.lock().await;
            if let Some(until) = *cd {
                if Instant::now() < until {
                    return Vec::new();
                }
            }
        }

        let offset = ((page.saturating_sub(1)) * 10) + 1;
        let encoded_q = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        let url = format!("https://search.yahoo.com/search?p={}&b={}", encoded_q, offset);

        let (code, html_str) = self.fetch_via_curl(&url).await;
        if code != 200 || html_str.is_empty() {
            tracing::debug!("Yahoo search returned HTTP {}, cooling down 60s", code);
            *self.yahoo_cooldown_until.lock().await =
                Some(Instant::now() + Duration::from_secs(60));
            return Vec::new();
        }

        let mut items = Vec::new();
        {
            let doc = scraper::Html::parse_document(&html_str);
            let card_sel = scraper::Selector::parse("div.algo").unwrap();
            let link_sel = scraper::Selector::parse("div.compTitle a, a[data-matarget='algo'], h3 a").unwrap();
            let h3_sel = scraper::Selector::parse("h3").unwrap();
            let snip_sel = scraper::Selector::parse("div.compText, p.lh-16, div.algo-desc").unwrap();

            for card in doc.select(&card_sel) {
                if let Some(link) = card.select(&link_sel).next() {
                    if let Some(href) = link.value().attr("href") {
                        let decoded = decode_yahoo_redirect(href);
                        if decoded.starts_with("http") {
                            if let Some(domain) = extract_domain(&decoded) {
                                if !is_excluded_domain(&domain) {
                                    let title = card
                                        .select(&h3_sel)
                                        .next()
                                        .map(|h| h.text().collect::<Vec<_>>().join(" ").trim().to_string())
                                        .filter(|t| !t.is_empty())
                                        .unwrap_or_else(|| link.text().collect::<Vec<_>>().join(" ").trim().to_string());

                                    let snippet = card
                                        .select(&snip_sel)
                                        .next()
                                        .map(|s| s.text().collect::<Vec<_>>().join(" ").trim().to_string())
                                        .unwrap_or_default();

                                    items.push(SearchItem {
                                        url: decoded,
                                        title,
                                        snippet,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        items
    }

    async fn search_brave_web(&self, query: &str, page: u32) -> Vec<SearchItem> {
        {
            let cd = self.brave_cooldown_until.lock().await;
            if let Some(until) = *cd {
                if Instant::now() < until {
                    return Vec::new();
                }
            }
        }

        let offset = page.saturating_sub(1);
        let encoded_q = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        let url = format!(
            "https://search.brave.com/search?q={}&offset={}&spellcheck=0",
            encoded_q, offset
        );

        let (code, html_str) = self.fetch_via_curl(&url).await;
        if code == 429 {
            tracing::debug!("Brave search returned HTTP 429, cooling down 90s");
            *self.brave_cooldown_until.lock().await =
                Some(Instant::now() + Duration::from_secs(90));
            return Vec::new();
        }
        if code != 200 || html_str.is_empty() {
            return Vec::new();
        }

        let mut items = Vec::new();
        {
            let doc = scraper::Html::parse_document(&html_str);
            let card_sel = scraper::Selector::parse("div[data-type='web']").unwrap();
            let link_sel = scraper::Selector::parse("a.l1, a[href^='http']").unwrap();
            let title_sel = scraper::Selector::parse("div.search-snippet-title, div.title").unwrap();
            let snip_sel =
                scraper::Selector::parse("div.generic-snippet div.content, div.snippet-description").unwrap();

            for card in doc.select(&card_sel) {
                if let Some(link) = card.select(&link_sel).next() {
                    if let Some(href) = link.value().attr("href") {
                        if href.starts_with("http") {
                            if let Some(domain) = extract_domain(href) {
                                if !is_excluded_domain(&domain) {
                                    let title = card
                                        .select(&title_sel)
                                        .next()
                                        .map(|t| t.text().collect::<Vec<_>>().join(" ").trim().to_string())
                                        .filter(|t| !t.is_empty())
                                        .unwrap_or_else(|| {
                                            link.text().collect::<Vec<_>>().join(" ").trim().to_string()
                                        });

                                    let snippet = card
                                        .select(&snip_sel)
                                        .next()
                                        .map(|s| s.text().collect::<Vec<_>>().join(" ").trim().to_string())
                                        .unwrap_or_default();

                                    items.push(SearchItem {
                                        url: href.to_string(),
                                        title,
                                        snippet,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        items
    }

    async fn search_bing_web(&self, query: &str, page: u32) -> Vec<SearchItem> {
        let first = ((page.saturating_sub(1)) * 10) + 1;
        let encoded_q = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        let url = format!(
            "https://www.bing.com/search?q={}&cc=ID&setlang=id&first={}",
            encoded_q, first
        );

        let (code, html_str) = self.fetch_via_curl(&url).await;
        if code != 200 || html_str.is_empty() {
            return Vec::new();
        }

        let mut items = Vec::new();
        {
            let doc = scraper::Html::parse_document(&html_str);
            let card_sel = scraper::Selector::parse("li.b_algo").unwrap();
            let link_sel = scraper::Selector::parse("h2 a").unwrap();
            let snip_sel = scraper::Selector::parse("div.b_caption p, p").unwrap();

            for card in doc.select(&card_sel) {
                if let Some(link) = card.select(&link_sel).next() {
                    if let Some(href) = link.value().attr("href") {
                        let decoded = decode_bing_redirect(href);
                        if decoded.starts_with("http") {
                            if let Some(domain) = extract_domain(&decoded) {
                                if !is_excluded_domain(&domain) {
                                    let title = link.text().collect::<Vec<_>>().join(" ").trim().to_string();
                                    let snippet = card
                                        .select(&snip_sel)
                                        .next()
                                        .map(|s| s.text().collect::<Vec<_>>().join(" ").trim().to_string())
                                        .unwrap_or_default();

                                    if !title.is_empty() {
                                        items.push(SearchItem {
                                            url: decoded,
                                            title,
                                            snippet,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        items
    }

    fn parse_indonetwork_html(html_str: &str, filter_kws: &[String]) -> Vec<SearchItem> {
        let mut items = Vec::new();
        let doc = scraper::Html::parse_document(html_str);
        let card_sel = scraper::Selector::parse("article.pcard").unwrap();
        let prod_link_sel = scraper::Selector::parse("h3.pcard__name a").unwrap();
        let sup_name_sel = scraper::Selector::parse(".pcard__supplier-name").unwrap();
        let place_sel = scraper::Selector::parse(".pcard__supplier-place").unwrap();
        let fact_sel = scraper::Selector::parse(".pcard__fact").unwrap();

        for card in doc.select(&card_sel) {
            let Some(link) = card.select(&prod_link_sel).next() else {
                continue;
            };
            let Some(href) = link.value().attr("href") else {
                continue;
            };
            let full_url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("https://www.indonetwork.co.id{}", href)
            };
            let prod_title = link.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let supplier_name = card
                .select(&sup_name_sel)
                .next()
                .map(|s| s.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .unwrap_or_default();
            let place = card
                .select(&place_sel)
                .next()
                .map(|p| p.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .unwrap_or_default();
            let facts: Vec<String> = card
                .select(&fact_sel)
                .map(|f| {
                    f.text()
                        .collect::<Vec<_>>()
                        .join(": ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();

            let check_blob = format!("{} {} {}", prod_title, facts.join(" "), href).to_lowercase();
            let matched = filter_kws.iter().filter(|k| check_blob.contains(k.as_str())).count();
            if matched < filter_kws.len().min(2) {
                continue;
            }

            let title = if supplier_name.is_empty() {
                prod_title.clone()
            } else {
                format!("{} - {}", prod_title, supplier_name)
            };
            let snippet = format!(
                "{} | {} | Indonesia | {}",
                supplier_name,
                place,
                facts.join(" | ")
            );

            items.push(SearchItem {
                url: full_url,
                title,
                snippet,
            });
        }
        items
    }

    async fn search_indonetwork(&self, query: &str, page: u32) -> Vec<SearchItem> {
        let keywords = clean_b2b_keywords(query);
        if keywords.is_empty() {
            return Vec::new();
        }

        let mut variants: Vec<Vec<String>> = vec![keywords.clone()];
        if keywords.len() >= 3 && page == 1 {
            variants.push(keywords[1..3].to_vec());
            variants.push(keywords[0..2].to_vec());
        }

        let mut collected = Vec::new();
        let mut seen = HashSet::new();

        for sub_kws in variants {
            let short_q = sub_kws.join(" ");
            let encoded_q = url::form_urlencoded::byte_serialize(short_q.as_bytes()).collect::<String>();
            let url = format!(
                "https://www.indonetwork.co.id/search?q={}&page={}",
                encoded_q, page
            );

            let (code, html_str) = self.fetch_via_curl(&url).await;
            if code == 200 && !html_str.is_empty() {
                for item in Self::parse_indonetwork_html(&html_str, &sub_kws) {
                    if seen.insert(item.url.clone()) {
                        collected.push(item);
                    }
                }
            }
            if collected.len() >= 5 {
                break;
            }
        }

        collected
    }
}

fn push_unique_items(
    results: &mut Vec<SearchItem>,
    seen_keys: &mut HashSet<String>,
    items: Vec<SearchItem>,
) {
    for item in items {
        if let Some(domain) = extract_domain(&item.url) {
            let key = if crate::services::entity_resolution::is_shared_supplier_platform(&domain) {
                item.url.clone()
            } else {
                domain
            };
            if seen_keys.insert(key) {
                results.push(item);
            }
        }
    }
}

#[async_trait]
impl SearchProvider for LiveSearchProvider {
    async fn search(&self, query: &str, page: u32) -> anyhow::Result<SearchPage> {
        let mut results = Vec::new();
        let mut seen_keys = HashSet::new();

        // Polite spacing between search engine requests to avoid burst rate limits
        tokio::time::sleep(Duration::from_millis(600)).await;

        // 1. Primary Global Web Index: Brave Search
        let brave = self.search_brave_web(query, page).await;
        let brave_n = brave.len();
        push_unique_items(&mut results, &mut seen_keys, brave);

        // 2. Backup Global Web Index: Yahoo Search
        let mut yahoo_n = None;
        if results.len() < 5 {
            let yahoo = self.search_yahoo(query, page).await;
            yahoo_n = Some(yahoo.len());
            push_unique_items(&mut results, &mut seen_keys, yahoo);
        }

        // 3. Backup Global Web Index: Bing Search
        let mut bing_n = None;
        if results.len() < 5 {
            let bing = self.search_bing_web(query, page).await;
            bing_n = Some(bing.len());
            push_unique_items(&mut results, &mut seen_keys, bing);
        }

        // 4. Always include Indonesian B2B Directory (Indonetwork) for Indonesia/Indonesian queries or scarce web hits
        let q_lower = query.to_lowercase();
        let mut indonetwork_n = None;
        if q_lower.contains("indonesia")
            || q_lower.contains("pabrik")
            || q_lower.contains("jual")
            || q_lower.contains("grosir")
            || results.len() < 10
        {
            let directory = self.search_indonetwork(query, page).await;
            indonetwork_n = Some(directory.len());
            push_unique_items(&mut results, &mut seen_keys, directory);
        }

        tracing::info!(
            "search '{}' p{}: brave={} yahoo={:?} bing={:?} indonetwork={:?} -> {} unique",
            query, page, brave_n, yahoo_n, bing_n, indonetwork_n, results.len()
        );

        let has_next_page = page <= 2 && !results.is_empty();

        Ok(SearchPage {
            query: query.to_string(),
            page_number: page,
            results,
            has_next_page,
        })
    }

    fn provider_name(&self) -> &str {
        "live"
    }
}

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

        // All URLs in this provider MUST be real, resolvable, and accessible websites!
        let results = if q_lower.contains("botol")
            || q_lower.contains("plastik")
            || q_lower.contains("cap")
            || q_lower.contains("bottle")
            || q_lower.contains("tutup")
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
                ]
            } else {
                vec![
                    SearchItem {
                        url: "https://www.indopackaging.com/katalog-tutup-botol".to_string(),
                        title: "Indo Packaging Solusindo - Grosir Tutup Botol dan Kemasan Plastik".to_string(),
                        snippet: "Menyediakan tutup botol plastik berbagai ukuran (28mm, 30mm, 38mm) siap kirim seluruh Indonesia.".to_string(),
                    },
                ]
            }
        } else if q_lower.contains("gelas") || q_lower.contains("cup") || q_lower.contains("organik") {
            if page == 1 {
                vec![
                    SearchItem {
                        url: "https://suryaindoplastic.id/supplier-gelas-plastik-eco-friendly-surya-indo-plastic/".to_string(),
                        title: "PT Surya Indo Plastic - Pabrik Gelas Minum Ramah Lingkungan & Eco Friendly".to_string(),
                        snippet: "Produsen kemasan gelas plastik dan paper cup eco-friendly untuk industri F&B dan perhotelan nasional.".to_string(),
                    },
                    SearchItem {
                        url: "https://ecorasa.id/".to_string(),
                        title: "ecorasa - Kotak Makanan dan Cup Minum Ramah Lingkungan Organik".to_string(),
                        snippet: "Solusi kemasan ramah lingkungan degradable oxium dan ecoplast untuk kebutuhan industri cafe dan restoran.".to_string(),
                    },
                    SearchItem {
                        url: "https://yesorganik.id/".to_string(),
                        title: "Yes Organik Indonesia - Produk Organik & Kemasan Alami Ramah Lingkungan".to_string(),
                        snippet: "Penyedia wadah dan gelas minum berbahan organik serta ramah lingkungan berkualitas food grade.".to_string(),
                    },
                ]
            } else {
                vec![
                    SearchItem {
                        url: "https://sealindonesia.com/id/supplier-gelas-plastik".to_string(),
                        title: "SEAL Indonesia - Supplier Resmi Gelas Plastik & Paper Cup Food Grade".to_string(),
                        snippet: "Pusat pasokan gelas PET dan PP standar higienis dengan berbagai kapasitas dan ukuran industri.".to_string(),
                    },
                    SearchItem {
                        url: "https://www.zehainter.com/".to_string(),
                        title: "PT ZEHA Internasional Abadi - Pabrik Pembuatan Gelas dan Kemasan".to_string(),
                        snippet: "Manufaktur wadah minuman, gelas plastik, dan kemasan cetak custom untuk sektor komersial.".to_string(),
                    },
                ]
            }
        } else if q_lower.contains("kipas") || q_lower.contains("fan") || q_lower.contains("angin") {
            if page == 1 {
                vec![
                    SearchItem {
                        url: "https://kdk.co.id/".to_string(),
                        title: "KDK Indonesia Official - Solusi Kipas Angin dan Ventilasi Industri".to_string(),
                        snippet: "Pabrik dan distributor resmi KDK Indonesia untuk exhaust fan, stand fan, ceiling fan, dan sirkulasi udara pabrik.".to_string(),
                    },
                    SearchItem {
                        url: "https://www.kipascke.co.id/".to_string(),
                        title: "PT Industrial Multi Fan (CKE) - Pusat Kipas Angin Industri & Gedung".to_string(),
                        snippet: "Supplier kipas angin industri terlengkap, wall fan komersial, drum fan, dan blower pabrik bergaransi resmi.".to_string(),
                    },
                    SearchItem {
                        url: "https://www.sinarmurni.com/".to_string(),
                        title: "Sinar Murni - Distributor Resmi Kipas Angin Komersial & Industri".to_string(),
                        snippet: "Pusat distribusi kipas angin berbagai merk ternama Panasonic, KDK, dan CKE untuk pengadaan proyek dan retail.".to_string(),
                    },
                ]
            } else {
                vec![
                    SearchItem {
                        url: "https://www.kreasiutamateknik.com/".to_string(),
                        title: "PT Kreasi Utama Teknik - Pabrik & Manufaktur Kipas Angin Industri Khusus".to_string(),
                        snippet: "Produsen industrial ventilation, custom blower, and heavy-duty fan solutions untuk pabrik di seluruh Indonesia.".to_string(),
                    },
                    SearchItem {
                        url: "https://www.scnp.co.id/".to_string(),
                        title: "PT Selaras Citra Nusantara Perkasa Tbk (SCNP) - Manufaktur Kipas Angin Nasional".to_string(),
                        snippet: "Produsen alat rumah tangga dan kipas angin berkualitas tinggi dengan fasilitas pabrik terakreditasi ISO.".to_string(),
                    },
                ]
            }
        } else if q_lower.contains("bearing")
            || q_lower.contains("skf")
            || q_lower.contains("timken")
            || q_lower.contains("fag")
            || q_lower.contains("nsk")
        {
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
        } else {
            // Real verified B2B directory links for arbitrary queries
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

            vec![
                SearchItem {
                    url: format!("https://www.indotrading.com/search?q={}", clean_slug),
                    title: format!("Direktori Supplier & Pabrik {} Indonesia - Indotrading", capitalized),
                    snippet: format!("Daftar produsen, importir, dan distributor terverifikasi untuk kebutuhan {}.", query),
                },
                SearchItem {
                    url: format!("https://www.ralali.com/v/{}", clean_slug),
                    title: format!("B2B Marketplace Grosir {} - Ralali B2B", capitalized),
                    snippet: format!("Pusat pengadaan B2B corporate untuk {} dengan harga grosir langsung dari supplier.", query),
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
        let p_name = self.provider_name.to_lowercase();
        let mut req = self.client.get(&self.base_url);

        if p_name.contains("serpapi") {
            let start = (page.saturating_sub(1)) * 10;
            req = req.query(&[
                ("q", query),
                ("api_key", &self.api_key),
                ("engine", "google"),
                ("start", &start.to_string()),
            ]);
        } else if p_name.contains("brave") {
            let offset = (page.saturating_sub(1)) * 10;
            req = req
                .query(&[("q", query), ("offset", &offset.to_string())])
                .header("X-Subscription-Token", &self.api_key);
        } else if p_name.contains("searxng") {
            req = req.query(&[
                ("q", query),
                ("format", "json"),
                ("pageno", &page.to_string()),
            ]);
        } else {
            req = req.query(&[
                ("q", query),
                ("page", &page.to_string()),
                ("key", &self.api_key),
            ]);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "HTTP search provider {} returned error {}: {}",
                self.provider_name,
                status,
                body
            ));
        }

        let resp_json: serde_json::Value = resp.json().await?;
        let mut results = Vec::new();

        let items_opt = resp_json
            .get("organic_results")
            .or_else(|| resp_json.get("results"))
            .or_else(|| resp_json.get("items"))
            .or_else(|| resp_json.get("web").and_then(|w| w.get("results")));

        if let Some(items) = items_opt {
            if let Some(arr) = items.as_array() {
                for it in arr {
                    let url = it
                        .get("link")
                        .or_else(|| it.get("url"))
                        .and_then(|u| u.as_str())
                        .unwrap_or_default();

                    let title = it
                        .get("title")
                        .or_else(|| it.get("name"))
                        .and_then(|t| t.as_str())
                        .unwrap_or_default();

                    let snippet = it
                        .get("snippet")
                        .or_else(|| it.get("description"))
                        .or_else(|| it.get("content"))
                        .and_then(|s| s.as_str())
                        .unwrap_or_default();

                    if !url.is_empty() {
                        results.push(SearchItem {
                            url: url.to_string(),
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
        "brave" => {
            let api_key = if !config.brave_search_api_key.is_empty() {
                config.brave_search_api_key.clone()
            } else {
                config.search_api_key.clone()
            };
            let base = if !config.search_api_base_url.is_empty() {
                config.search_api_base_url.clone()
            } else {
                "https://api.search.brave.com/res/v1/web/search".to_string()
            };
            if api_key.is_empty() {
                tracing::info!("Brave API key not configured, using Live search engine");
                Arc::new(LiveSearchProvider::new())
            } else {
                Arc::new(HttpSearchProvider::new(
                    base,
                    api_key,
                    "brave".to_string(),
                ))
            }
        }
        "http" | "serpapi" | "searxng" | "google" => {
            if config.search_api_base_url.is_empty() {
                tracing::info!("External search API base URL not provided, using Live search engine");
                Arc::new(LiveSearchProvider::new())
            } else {
                Arc::new(HttpSearchProvider::new(
                    config.search_api_base_url.clone(),
                    config.search_api_key.clone(),
                    config.search_provider.clone(),
                ))
            }
        }
        "mock" => Arc::new(MockSearchProvider),
        // Live search is default!
        _ => Arc::new(LiveSearchProvider::new()),
    }
}
