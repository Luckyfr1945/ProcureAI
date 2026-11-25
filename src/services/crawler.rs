use crate::utils::text::{
    calculate_content_hash, clean_text, extract_phones_and_whatsapp, extract_valid_emails,
    extract_whatsapp_from_href,
};
use crate::utils::url_safety::validate_url_safety;
use reqwest::Client;
use reqwest::header::CONTENT_TYPE;
use scraper::{Html, Selector};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CrawlResult {
    pub original_url: String,
    pub final_url: String,
    pub http_status: u16,
    pub page_title: String,
    pub site_name: Option<String>,
    pub meta_description: Option<String>,
    pub clean_text: String,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub content_hash: String,
}

#[derive(Clone)]
pub struct AsyncCrawler {
    timeout_seconds: u64,
    max_response_bytes: usize,
}

impl AsyncCrawler {
    pub fn new(timeout_seconds: u64, max_response_bytes: usize) -> Self {
        Self {
            timeout_seconds,
            max_response_bytes,
        }
    }

    pub async fn fetch_page(&self, target_url: &str) -> anyhow::Result<CrawlResult> {
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(self.timeout_seconds.max(1));
        let mut current = target_url.to_string();
        let mut response = None;
        for _ in 0..=5 {
            let safe_url =
                tokio::time::timeout_at(deadline, validate_url_safety(&current)).await??;
            let host = safe_url
                .host_str()
                .ok_or_else(|| anyhow::anyhow!("Missing host"))?
                .trim_matches(&['[', ']'][..]);
            // Resolve, validate and pin the exact addresses used by this hop; disable proxies.
            let addresses: Vec<std::net::SocketAddr> = tokio::time::timeout_at(
                deadline,
                tokio::net::lookup_host((host, safe_url.port_or_known_default().unwrap_or(80))),
            )
            .await??
            .collect();
            anyhow::ensure!(
                !addresses.is_empty()
                    && addresses
                        .iter()
                        .all(|a| !crate::utils::url_safety::is_private_ip(&a.ip())),
                "Blocked destination address"
            );
            let client = Client::builder()
                .timeout(Duration::from_secs(self.timeout_seconds.max(1)))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .user_agent("ProcureAI-Discovery/1.0")
                .resolve_to_addrs(host, &addresses)
                .build()?;
            let resp =
                tokio::time::timeout_at(deadline, client.get(safe_url.clone()).send()).await??;
            if resp.status().is_redirection() {
                let location = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .ok_or_else(|| anyhow::anyhow!("Missing redirect location"))?
                    .to_str()?;
                current = safe_url.join(location)?.to_string();
                continue;
            }
            response = Some(resp);
            break;
        }
        let mut resp = response.ok_or_else(|| anyhow::anyhow!("Too many redirects"))?;
        let status = resp.status();
        let final_url = resp.url().to_string();
        anyhow::ensure!(status.is_success(), "HTTP error status: {}", status);
        if let Some(ct) = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
        {
            let lower = ct.to_lowercase();
            anyhow::ensure!(
                lower.contains("text/html") || lower.contains("application/xhtml"),
                "Skipping non-HTML content-type"
            );
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = tokio::time::timeout_at(deadline, resp.chunk()).await?? {
            anyhow::ensure!(
                bytes.len().saturating_add(chunk.len()) <= self.max_response_bytes,
                "Response exceeds byte limit"
            );
            bytes.extend_from_slice(&chunk);
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

        // Meta description & og:site_name
        let meta_selector =
            Selector::parse(r#"meta[name="description"], meta[property="og:description"]"#)
                .unwrap();
        let meta_description = document
            .select(&meta_selector)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| clean_text(s));

        let site_name_selector =
            Selector::parse(r#"meta[property="og:site_name"], meta[name="application-name"]"#)
                .unwrap();
        let site_name = document
            .select(&site_name_selector)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(clean_text)
            .filter(|s| s.len() >= 2 && s.len() <= 80);

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
        let cleaned_text = text_parts.join("\n");

        // Extract mailto:, tel:, and WhatsApp links
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
                } else if let Some(wa_phone) = extract_whatsapp_from_href(href) {
                    if !phones.contains(&wa_phone) {
                        phones.push(wa_phone);
                    }
                }
            }
        }

        // Also scan body text for valid email addresses and WhatsApp / phone numbers
        for email in extract_valid_emails(&cleaned_text) {
            if !emails.contains(&email) {
                emails.push(email);
            }
        }
        for phone in extract_phones_and_whatsapp(&cleaned_text) {
            if !phones.contains(&phone) {
                phones.push(phone);
            }
        }

        Ok(CrawlResult {
            original_url: target_url.to_string(),
            final_url,
            http_status: status.as_u16(),
            page_title,
            site_name,
            meta_description,
            clean_text: cleaned_text,
            emails,
            phones,
            content_hash,
        })
    }
}
