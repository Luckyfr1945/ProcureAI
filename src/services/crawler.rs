use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use scraper::{Html, Selector};
use std::time::Duration;
use crate::utils::text::{calculate_content_hash, clean_text, extract_valid_emails};
use crate::utils::url_safety::validate_url_safety;

#[derive(Debug, Clone)]
pub struct CrawlResult {
    pub original_url: String,
    pub final_url: String,
    pub http_status: u16,
    pub page_title: String,
    pub meta_description: Option<String>,
    pub clean_text: String,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub content_hash: String,
}

#[derive(Clone)]
pub struct AsyncCrawler {
    client: Client,
    max_response_bytes: usize,
}

impl AsyncCrawler {
    pub fn new(timeout_seconds: u64, max_response_bytes: usize) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .user_agent("ProcureAI-Discovery/1.0 (Procurement Research Bot; polite; +https://internal.company/procureai)")
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("Too many redirects")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .expect("Failed to build crawler HTTP client");

        Self {
            client,
            max_response_bytes,
        }
    }

    pub async fn fetch_page(&self, target_url: &str) -> anyhow::Result<CrawlResult> {
        // 1. SSRF prevention check
        let safe_url = validate_url_safety(target_url).await?;

        // 2. Perform HTTP GET
        let resp = self.client.get(safe_url.as_str()).send().await?;
        let status = resp.status();
        let final_url = resp.url().to_string();

        // 3. Check for SSRF on final redirected URL as well
        if final_url != target_url {
            let _ = validate_url_safety(&final_url).await?;
        }

        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP error status: {}", status));
        }

        // 4. Validate content-type is text/html
        if let Some(ct) = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
            let ct_lower = ct.to_lowercase();
            if !ct_lower.contains("text/html") && !ct_lower.contains("application/xhtml") {
                return Err(anyhow::anyhow!("Skipping non-HTML content-type: {}", ct));
            }
        }

        // 5. Read response body bounded by max_bytes
        let bytes = resp.bytes().await?;
        if bytes.len() > self.max_response_bytes {
            return Err(anyhow::anyhow!(
                "Response size {} exceeds limit of {} bytes",
                bytes.len(),
                self.max_response_bytes
            ));
        }

        let html_str = String::from_utf8_lossy(&bytes).to_string();
        let content_hash = calculate_content_hash(&html_str);

        // 6. Parse HTML
        let document = Html::parse_document(&html_str);

        // Title
        let title_selector = Selector::parse("title").unwrap();
        let page_title = document
            .select(&title_selector)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join(" "))
            .map(|t| clean_text(&t))
            .unwrap_or_default();

        // Meta description
        let meta_selector =
            Selector::parse(r#"meta[name="description"], meta[property="og:description"]"#)
                .unwrap();
        let meta_description = document
            .select(&meta_selector)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| clean_text(s));

        // Extract body text while strictly excluding <script>, <style>, <noscript>, <svg>
        let body_selector = Selector::parse("body").unwrap();
        let mut text_parts = Vec::new();
        if let Some(body_el) = document.select(&body_selector).next() {
            for node in body_el.descendants() {
                if let Some(text) = node.value().as_text() {
                    let has_script_or_style = node.ancestors().any(|anc| {
                        if let Some(el) = anc.value().as_element() {
                            let name = el.name();
                            name == "script"
                                || name == "style"
                                || name == "noscript"
                                || name == "svg"
                                || name == "head"
                        } else {
                            false
                        }
                    });

                    if !has_script_or_style {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            text_parts.push(trimmed);
                        }
                    }
                }
            }
        }
        let cleaned_text = text_parts.join(" ");

        // Extract mailto: and tel: links
        let link_selector = Selector::parse("a[href]").unwrap();
        let mut emails = Vec::new();
        let mut phones = Vec::new();

        for link in document.select(&link_selector) {
            if let Some(href) = link.value().attr("href") {
                if let Some(email) = href.strip_prefix("mailto:") {
                    let clean_email = email
                        .split('?')
                        .next()
                        .unwrap_or(email)
                        .trim()
                        .to_lowercase();
                    for valid in extract_valid_emails(&clean_email) {
                        if !emails.contains(&valid) {
                            emails.push(valid);
                        }
                    }
                } else if let Some(phone) = href.strip_prefix("tel:") {
                    let clean_phone = phone
                        .chars()
                        .filter(|c| c.is_numeric() || *c == '+' || *c == '-' || *c == ' ')
                        .collect::<String>()
                        .trim()
                        .to_string();
                    if clean_phone.len() >= 6 && !phones.contains(&clean_phone) {
                        phones.push(clean_phone);
                    }
                }
            }
        }

        // Also scan body text for valid email addresses
        for email in extract_valid_emails(&cleaned_text) {
            if !emails.contains(&email) {
                emails.push(email);
            }
        }

        Ok(CrawlResult {
            original_url: target_url.to_string(),
            final_url,
            http_status: status.as_u16(),
            page_title,
            meta_description,
            clean_text: cleaned_text,
            emails,
            phones,
            content_hash,
        })
    }
}
