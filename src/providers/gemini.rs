use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedQueryOutput {
    pub normalized_requirement: String,
    pub important_specs: Vec<String>,
    pub search_queries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedProductInfo {
    pub product_name: String,
    pub brand: Option<String>,
    pub model_or_specification: Option<String>,
    pub price_amount: Option<f64>,
    pub price_currency: Option<String>,
    pub minimum_order_quantity: Option<String>,
    pub availability_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSupplierInfo {
    pub company_name: Option<String>,
    pub website: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub products: Vec<ExtractedProductInfo>,
}

#[derive(Clone)]
pub struct GeminiClient {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn new(api_key: String, model: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            api_key,
            model,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    pub async fn expand_search_query(
        &self,
        request: &str,
        max_queries: usize,
    ) -> anyhow::Result<ExpandedQueryOutput> {
        if !self.is_configured() {
            return Ok(self.fallback_query_expansion(request, max_queries));
        }

        let prompt = format!(
            r#"You are a professional industrial procurement expert.
Analyze the user's procurement request and generate structured search queries for external search engines to find manufacturers and distributors worldwide.

Rules:
1. Do NOT alter product types, model numbers, or technical dimensions.
2. Do NOT invent fake supplier names.
3. Return search queries, not fake supplier results.
4. Output strict JSON with keys:
   - "normalized_requirement": concise technical description of the requested item
   - "important_specs": array of key technical identifiers (e.g. brand, model numbers)
   - "search_queries": array of up to {} search engine query strings (e.g. "<item> supplier", "<item> authorized distributor", "<item> wholesale manufacturer")

User request: "{}"
"#,
            max_queries, request
        );

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let req_body = serde_json::json!({
            "contents": [{
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "responseMimeType": "application/json",
                "temperature": 0.2
            }
        });

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            tracing::warn!("Gemini API error ({}): {}, falling back to heuristic query expansion", status, err_text);
            return Ok(self.fallback_query_expansion(request, max_queries));
        }

        let resp_json: serde_json::Value = resp.json().await?;
        let text = resp_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid response structure from Gemini API"))?;

        let parsed: ExpandedQueryOutput = serde_json::from_str(text)?;
        Ok(parsed)
    }

    pub fn fallback_query_expansion(
        &self,
        request: &str,
        max_queries: usize,
    ) -> ExpandedQueryOutput {
        let clean_req = request.trim();
        let words: Vec<&str> = clean_req.split_whitespace().collect();

        // Extract key terms that look like models or capitalized specs
        let mut important_specs: Vec<String> = words
            .iter()
            .filter(|w| w.chars().any(|c| c.is_numeric()) || w.len() > 2 && w.chars().all(|c| c.is_uppercase()))
            .map(|w| w.to_string())
            .collect();

        if important_specs.is_empty() && !words.is_empty() {
            important_specs.push(words[0].to_string());
        }

        let mut queries = vec![
            format!("{} supplier", clean_req),
            format!("{} authorized distributor", clean_req),
            format!("{} manufacturer catalog", clean_req),
            format!("{} wholesale exporter", clean_req),
        ];
        queries.truncate(max_queries);

        ExpandedQueryOutput {
            normalized_requirement: clean_req.to_string(),
            important_specs,
            search_queries: queries,
        }
    }

    pub async fn extract_supplier_info(
        &self,
        page_url: &str,
        page_title: &str,
        page_text: &str,
    ) -> anyhow::Result<ExtractedSupplierInfo> {
        if !self.is_configured() {
            return Ok(self.fallback_supplier_extraction(page_url, page_title, page_text));
        }

        // Limit page text size for prompt safety
        let truncated_text: String = page_text.chars().take(8000).collect();

        let prompt = format!(
            r#"Analyze the following public webpage text from URL "{}" with title "{}".
Extract supplier company information and any offered products related to procurement.

Strict rules:
1. Only extract information explicitly mentioned in the text.
2. If email, phone, city, or country are not mentioned, return null. Do NOT guess or invent.
3. For products, extract product name, specifications, and prices only if stated.
4. Output strict JSON matching this structure:
{{
  "company_name": "string or null",
  "website": "string or null",
  "country": "string or null",
  "city": "string or null",
  "email": "string or null",
  "phone": "string or null",
  "category": "string or null",
  "description": "short summary or null",
  "products": [
    {{
      "product_name": "string",
      "brand": "string or null",
      "model_or_specification": "string or null",
      "price_amount": 100.0 or null,
      "price_currency": "USD" or null,
      "minimum_order_quantity": "string or null",
      "availability_note": "string or null"
    }}
  ]
}}

Webpage content:
{}
"#,
            page_url, page_title, truncated_text
        );

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let req_body = serde_json::json!({
            "contents": [{
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "responseMimeType": "application/json",
                "temperature": 0.1
            }
        });

        let resp = self.client.post(&url).json(&req_body).send().await?;

        if !resp.status().is_success() {
            return Ok(self.fallback_supplier_extraction(page_url, page_title, page_text));
        }

        let resp_json: serde_json::Value = resp.json().await?;
        let text = resp_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid response from Gemini"))?;

        let parsed: ExtractedSupplierInfo = serde_json::from_str(text)?;
        Ok(parsed)
    }

    pub fn fallback_supplier_extraction(
        &self,
        page_url: &str,
        page_title: &str,
        page_text: &str,
    ) -> ExtractedSupplierInfo {
        // Fallback rule-based extractor
        let company_name = if !page_title.is_empty() {
            let name_part = page_title.split(&['|', '-', '—'][..]).next().unwrap_or(page_title);
            Some(name_part.trim().to_string())
        } else {
            None
        };

        // Extract email if present
        let email = page_text
            .split_whitespace()
            .find(|w| w.contains('@') && w.contains('.') && !w.contains("example.com"))
            .map(|w| w.trim_matches(&['(', ')', '<', '>', ',', ';', '"', '\''][..]).to_string());

        ExtractedSupplierInfo {
            company_name,
            website: Some(page_url.to_string()),
            country: None,
            city: None,
            email,
            phone: None,
            category: None,
            description: Some(page_title.to_string()),
            products: Vec::new(),
        }
    }
}

