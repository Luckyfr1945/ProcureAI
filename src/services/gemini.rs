use crate::domain::ProductRequirement;
use crate::services::location;
use crate::utils::text::{extract_phones_and_whatsapp, extract_valid_emails};
use crate::utils::url_safety::extract_domain;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedQuery {
    pub query_text: String,
    pub query_type: String,
    pub language: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedOfferPayload {
    pub product_name: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub part_number: Option<String>,
    pub description: Option<String>,
    pub attributes: serde_json::Value,
    pub price_amount: Option<f64>,
    pub currency: Option<String>,
    pub price_basis: Option<String>,
    pub moq: Option<String>,
    pub lead_time: Option<String>,
    pub availability_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEvidencePayload {
    pub field_path: String,
    pub field_value: serde_json::Value,
    pub evidence_quote: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSupplierPayload {
    pub company_name: Option<String>,
    pub website: Option<String>,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub address: Option<String>,
    pub supplier_type: Option<String>,
    pub business_category: Option<String>,
    pub public_email: Option<String>,
    pub public_phone: Option<String>,
    pub contact_page_url: Option<String>,
    #[serde(default)]
    pub offers: Vec<ExtractedOfferPayload>,
    #[serde(default)]
    pub evidence_items: Vec<ExtractedEvidencePayload>,
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
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to build reqwest client for Gemini");

        Self {
            client,
            api_key,
            model,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    /// Task 1: Requirement Analysis
    /// Converts raw natural language procurement requirement into category-aware ProductRequirement JSON.
    pub async fn analyze_requirement(
        &self,
        input_text: &str,
        target_regions: &[String],
        languages: &[String],
    ) -> anyhow::Result<ProductRequirement> {
        if !self.is_configured() {
            return Ok(self.fallback_analyze_requirement(input_text, target_regions, languages));
        }

        let prompt = format!(
            r#"You are an industrial procurement engineer.
Analyze the user's natural language procurement request and convert it into a structured ProductRequirement JSON.

Rules:
1. Identify the exact product name, category key (e.g. industrial_bearing, packaging_glass, electrical_cable, chemicals, office_supplies), brand, and part number/model if specified.
2. Determine category-specific required attributes (e.g. for bearing: inner_diameter_mm, outer_diameter_mm, width_mm; for cup/glass: material, volume_ml, eco_friendly_type).
3. Determine optional attributes (e.g. seal_type, clearance, certification, color).
4. Specify target supplier types (e.g. ["manufacturer", "authorized_distributor", "distributor", "wholesaler"]).
5. Specify search languages (e.g. ["en", "id"]).
6. Do NOT invent fake data. Return strict JSON.

User input: "{}"
Target regions: {:?}
Preferred languages: {:?}

Output JSON format:
{{
  "product_name": "string",
  "category": "string",
  "brand": "string or null",
  "part_number": "string or null",
  "synonyms": ["string"],
  "required_attributes": ["string"],
  "optional_attributes": ["string"],
  "supplier_types": ["string"],
  "target_regions": ["string"],
  "search_languages": ["string"]
}}
"#,
            input_text, target_regions, languages
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

        let resp = self.client.post(&url).json(&req_body).send().await;
        let resp = match resp {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                let err_text = r.text().await.unwrap_or_default();
                tracing::warn!(
                    "Gemini API analyze_requirement status {}: {}, using heuristic fallback",
                    status,
                    err_text
                );
                return Ok(self.fallback_analyze_requirement(
                    input_text,
                    target_regions,
                    languages,
                ));
            }
            Err(e) => {
                tracing::warn!("Gemini API request error: {}, using heuristic fallback", e);
                return Ok(self.fallback_analyze_requirement(
                    input_text,
                    target_regions,
                    languages,
                ));
            }
        };

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

        match serde_json::from_str::<ProductRequirement>(text) {
            Ok(mut req) => {
                if !target_regions.is_empty() {
                    req.target_regions = target_regions.to_vec();
                }
                if !languages.is_empty() {
                    req.search_languages = languages.to_vec();
                }
                Ok(req)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to deserialize Gemini requirement JSON: {}, using fallback",
                    e
                );
                Ok(self.fallback_analyze_requirement(input_text, target_regions, languages))
            }
        }
    }

    pub fn fallback_analyze_requirement(
        &self,
        input_text: &str,
        target_regions: &[String],
        languages: &[String],
    ) -> ProductRequirement {
        let clean = input_text.trim();
        let lower = clean.to_lowercase();
        let words: Vec<&str> = clean.split_whitespace().collect();

        let mut brand = None;
        let mut part_number = None;
        let mut category = "general_industrial".to_string();
        let mut required_attributes = vec!["specification".to_string()];
        let mut optional_attributes = vec!["moq".to_string(), "lead_time".to_string()];

        if lower.contains("bearing")
            || lower.contains("skf")
            || lower.contains("timken")
            || lower.contains("nsk")
        {
            category = "industrial_bearing".to_string();
            required_attributes = vec![
                "bearing_type".to_string(),
                "inner_diameter_mm".to_string(),
                "outer_diameter_mm".to_string(),
                "width_mm".to_string(),
            ];
            optional_attributes = vec!["seal_type".to_string(), "clearance".to_string()];
        } else if lower.contains("gelas")
            || lower.contains("cup")
            || lower.contains("organik")
            || lower.contains("plastik")
        {
            category = "packaging_beverage".to_string();
            required_attributes = vec!["material".to_string(), "volume_ml".to_string()];
            optional_attributes = vec![
                "eco_friendly_type".to_string(),
                "food_grade_cert".to_string(),
            ];
        } else if lower.contains("kipas") || lower.contains("fan") || lower.contains("blower") {
            category = "industrial_ventilation".to_string();
            required_attributes = vec![
                "fan_type".to_string(),
                "blade_diameter_inch".to_string(),
                "power_watt".to_string(),
            ];
            optional_attributes = vec!["voltage".to_string(), "airflow_cfm".to_string()];
        }

        // Detect brand keywords
        for w in &words {
            let lw = w.to_lowercase();
            if lw == "skf"
                || lw == "timken"
                || lw == "nsk"
                || lw == "fag"
                || lw == "kdk"
                || lw == "cke"
                || lw == "panasonic"
                || lw == "siemens"
                || lw == "schneider"
                || lw == "omron"
            {
                brand = Some(w.to_uppercase());
            }
            if w.chars().any(|c| c.is_numeric()) && w.len() >= 3 && !is_measurement_token(w) {
                part_number = Some(w.to_string());
            }
        }

        let regions = if target_regions.is_empty() {
            vec!["Indonesia".to_string()]
        } else {
            target_regions.to_vec()
        };

        let targets_id = regions.iter().any(|r| is_indonesia_region(r));
        let langs = if languages.is_empty() {
            if targets_id {
                vec!["id".to_string(), "en".to_string()]
            } else {
                vec!["en".to_string()]
            }
        } else {
            languages.to_vec()
        };

        ProductRequirement {
            product_name: clean.to_string(),
            category,
            brand,
            part_number,
            synonyms: vec![clean.to_string()],
            required_attributes,
            optional_attributes,
            supplier_types: vec![
                "manufacturer".to_string(),
                "authorized_distributor".to_string(),
                "distributor".to_string(),
                "wholesaler".to_string(),
            ],
            target_regions: regions,
            search_languages: langs,
            pr_number: None,
        }
    }

    /// Task 2: Query Planning
    /// Generates structured, diverse query batch from ProductRequirement.
    pub async fn plan_queries(
        &self,
        req: &ProductRequirement,
        max_queries: usize,
    ) -> anyhow::Result<Vec<PlannedQuery>> {
        if !self.is_configured() {
            return Ok(self.fallback_plan_queries(req, max_queries));
        }

        let prompt = format!(
            r#"You are a search query engineer for industrial supplier discovery.
Generate up to {} diverse, targeted search engine queries based on this requirement:

Product requirement:
{}

Rules:
1. Provide queries of distinct query_types:
   - "exact": exact product and part number
   - "manufacturer": official factory / manufacturer terms
   - "distributor": authorized distributor, stockist, wholesale supply
   - "specification": technical specifications or dimensions
   - "regional": supplier targeting specified regions
2. CRITICAL LOCATION RULE: Strictly target ONLY the specified target_regions in the ProductRequirement. If target_regions specifies a country outside Indonesia (such as China, Singapore, Japan, Germany), DO NOT include "indonesia" or Indonesian location keywords in the queries.
3. Follow the requested search_languages (e.g., use English if only "en" is specified, or Indonesian + English if "id" is included).
4. Output strict JSON array of objects:
[
  {{
    "query_text": "string",
    "query_type": "exact | manufacturer | distributor | specification | regional",
    "language": "en | id",
    "rationale": "short explanation"
  }}
]
"#,
            max_queries,
            serde_json::to_string_pretty(req)?
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

        let resp = match self.client.post(&url).json(&req_body).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => return Ok(self.fallback_plan_queries(req, max_queries)),
        };

        let resp_json: serde_json::Value = resp.json().await?;
        let text = resp_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("[]");

        match serde_json::from_str::<Vec<PlannedQuery>>(text) {
            Ok(queries) if !queries.is_empty() => {
                Ok(location::enforce_query_regions(queries, req, max_queries))
            }
            _ => Ok(self.fallback_plan_queries(req, max_queries)),
        }
    }

    pub fn fallback_plan_queries(
        &self,
        req: &ProductRequirement,
        max_queries: usize,
    ) -> Vec<PlannedQuery> {
        let name = compact_search_subject(&req.product_name);
        let mut list = Vec::new();

        let target_codes = location::target_country_codes(&req.target_regions);
        let subregions = location::target_subregions(&req.target_regions);
        let targets_indonesia = target_codes.iter().any(|c| c == "ID");
        let is_global = target_codes.is_empty();

        // Pick a clean single city/region token (e.g., "Surabaya" instead of "Surabaya, Gresik, Sidoarjo, Jawa Timur")
        let primary_city = location::primary_subregion_for_query(&subregions);
        let primary_country = target_codes
            .first()
            .map(|c| location::canonical_country_name(c))
            .unwrap_or("");
        let primary_region = primary_city.as_deref().unwrap_or(primary_country);

        let allow_id = req
            .search_languages
            .iter()
            .any(|l| l.eq_ignore_ascii_case("id"));
        if subregions.len() > 1 {
            let subject = req
                .part_number
                .as_deref()
                .map(|pn| format!("{} {}", req.brand.as_deref().unwrap_or(""), pn))
                .unwrap_or_else(|| name.clone());
            for region in &subregions {
                list.push(PlannedQuery {
                    query_text: format!("{} supplier {} {}", subject, region, primary_country),
                    query_type: "regional".into(),
                    language: "id,en".into(),
                    rationale: "Coverage for each selected city or region".into(),
                });
            }
        }

        if let Some(pn) = &req.part_number {
            let brand_str = req.brand.as_deref().unwrap_or("");
            let base = format!("{} {}", brand_str, pn).trim().to_string();

            if targets_indonesia {
                let loc_label = if let Some(ref city) = primary_city {
                    format!("{} Indonesia", city)
                } else {
                    "Indonesia".to_string()
                };
                list.push(PlannedQuery {
                    query_text: format!("{} supplier {}", base, loc_label)
                        .trim()
                        .to_string(),
                    query_type: "regional".to_string(),
                    language: if allow_id { "id" } else { "en" }.to_string(),
                    rationale: format!("Regional supplier lookup in {}", loc_label),
                });
                list.push(PlannedQuery {
                    query_text: format!("{} distributor resmi {}", base, loc_label),
                    query_type: "distributor".to_string(),
                    language: "id".to_string(),
                    rationale: "Authorized Indonesian distributor discovery".to_string(),
                });
                list.push(PlannedQuery {
                    query_text: format!("jual {} {} PT CV", base, loc_label),
                    query_type: "manufacturer".to_string(),
                    language: "id".to_string(),
                    rationale: "Indonesian industrial B2B supplier discovery".to_string(),
                });
                list.push(PlannedQuery {
                    query_text: format!("{} harga distributor Indonesia", base),
                    query_type: "specification".to_string(),
                    language: "id".to_string(),
                    rationale: "Indonesian distributor pricing & specification lookup".to_string(),
                });
            } else if !is_global {
                list.push(PlannedQuery {
                    query_text: format!("{} supplier manufacturer {}", base, primary_region),
                    query_type: "regional".to_string(),
                    language: "en".to_string(),
                    rationale: format!("Targeted supplier lookup in {}", primary_region),
                });
                list.push(PlannedQuery {
                    query_text: format!("{} factory exporter wholesale {}", base, primary_region),
                    query_type: "manufacturer".to_string(),
                    language: "en".to_string(),
                    rationale: format!("Factory and exporter search in {}", primary_region),
                });
                list.push(PlannedQuery {
                    query_text: format!("{} authorized distributor {}", base, primary_region),
                    query_type: "distributor".to_string(),
                    language: "en".to_string(),
                    rationale: format!("Distributor search in {}", primary_region),
                });
                list.push(PlannedQuery {
                    query_text: format!("{} datasheet specification {}", base, primary_region),
                    query_type: "specification".to_string(),
                    language: "en".to_string(),
                    rationale: "Technical specification lookup".to_string(),
                });
            } else {
                list.push(PlannedQuery {
                    query_text: format!("{} supplier manufacturer", base),
                    query_type: "exact".to_string(),
                    language: "en".to_string(),
                    rationale: "Global part number supplier lookup".to_string(),
                });
                list.push(PlannedQuery {
                    query_text: format!("{} authorized distributor wholesale", base),
                    query_type: "distributor".to_string(),
                    language: "en".to_string(),
                    rationale: "Global authorized distributor lookup".to_string(),
                });
                list.push(PlannedQuery {
                    query_text: format!("{} datasheet catalog specification", pn),
                    query_type: "specification".to_string(),
                    language: "en".to_string(),
                    rationale: "Technical specification lookup".to_string(),
                });
            }
        } else if targets_indonesia {
            let loc_label = if let Some(ref city) = primary_city {
                format!("{} Indonesia", city)
            } else {
                "Indonesia".to_string()
            };
            list.push(PlannedQuery {
                query_text: format!("{} supplier distributor {}", name, loc_label),
                query_type: "distributor".to_string(),
                language: if allow_id { "id" } else { "en" }.to_string(),
                rationale: format!("Distributor lookup in {}", loc_label),
            });
            list.push(PlannedQuery {
                query_text: format!("pabrik produsen {} {}", name, loc_label),
                query_type: "manufacturer".to_string(),
                language: "id".to_string(),
                rationale: format!("Manufacturer lookup in {}", loc_label),
            });
            list.push(PlannedQuery {
                query_text: format!("jual grosir {} Indonesia PT CV", name),
                query_type: "regional".to_string(),
                language: "id".to_string(),
                rationale: "B2B Indonesian company search".to_string(),
            });
            list.push(PlannedQuery {
                query_text: format!("{} supplier b2b {}", name, loc_label),
                query_type: "specification".to_string(),
                language: "id".to_string(),
                rationale: "B2B procurement supply directory".to_string(),
            });
        } else if !is_global {
            list.push(PlannedQuery {
                query_text: format!("{} manufacturer supplier {}", name, primary_region),
                query_type: "manufacturer".to_string(),
                language: "en".to_string(),
                rationale: format!("Direct manufacturer search in {}", primary_region),
            });
            list.push(PlannedQuery {
                query_text: format!("{} wholesale factory exporter {}", name, primary_region),
                query_type: "regional".to_string(),
                language: "en".to_string(),
                rationale: format!("Wholesale exporter search in {}", primary_region),
            });
            list.push(PlannedQuery {
                query_text: format!("{} distributor b2b {}", name, primary_region),
                query_type: "distributor".to_string(),
                language: "en".to_string(),
                rationale: format!("Distributor search in {}", primary_region),
            });
            list.push(PlannedQuery {
                query_text: format!("{} specification catalog {}", name, primary_region),
                query_type: "specification".to_string(),
                language: "en".to_string(),
                rationale: "Product catalog and technical specs".to_string(),
            });
        } else {
            list.push(PlannedQuery {
                query_text: format!("{} manufacturer supplier", name),
                query_type: "manufacturer".to_string(),
                language: "en".to_string(),
                rationale: "Global manufacturer lookup".to_string(),
            });
            list.push(PlannedQuery {
                query_text: format!("{} wholesale distributor exporter", name),
                query_type: "distributor".to_string(),
                language: "en".to_string(),
                rationale: "Global wholesale distributor search".to_string(),
            });
            list.push(PlannedQuery {
                query_text: format!("{} b2b industrial supplier", name),
                query_type: "regional".to_string(),
                language: "en".to_string(),
                rationale: "Global B2B directory search".to_string(),
            });
        }

        // Expand useful terms, preserving product identity and selected geography.
        let base = req
            .part_number
            .as_deref()
            .map(|pn| format!("{} {}", req.brand.as_deref().unwrap_or(""), pn))
            .unwrap_or_else(|| name.clone());
        for synonym in req.synonyms.iter().filter(|s| !s.trim().is_empty()).take(3) {
            list.push(PlannedQuery {
                query_text: format!(
                    "{} {} supplier {}",
                    synonym,
                    req.part_number.as_deref().unwrap_or(""),
                    primary_region
                ),
                query_type: "synonym".into(),
                language: "id,en".into(),
                rationale: "Alternate product terminology".into(),
            });
        }
        for suffix in [
            "supplier contact",
            "distributor katalog",
            "stockist ready stock",
            "dealer resmi",
            "supplier harga penawaran",
            "supplier perusahaan",
        ] {
            list.push(PlannedQuery {
                query_text: format!("{} {} {}", base, suffix, primary_region),
                query_type: "expanded".into(),
                language: "id,en".into(),
                rationale: "Additional supplier and catalog coverage".into(),
            });
        }
        let mut seen = std::collections::HashSet::new();
        let mut queries = location::enforce_query_regions(list, req, usize::MAX);
        queries.retain(|q| seen.insert(q.query_text.to_lowercase()));
        queries.truncate(max_queries);
        queries
    }

    /// Task 3: Structured Extraction with Verifiable Evidence Quotes
    /// Extracts supplier, offers, and verbatim evidence quotes supporting each field.
    pub async fn extract_supplier_and_offers(
        &self,
        page_url: &str,
        page_title: &str,
        page_text: &str,
        req: &ProductRequirement,
    ) -> anyhow::Result<ExtractedSupplierPayload> {
        if !self.is_configured() {
            return Ok(self.fallback_extract_supplier(page_url, page_title, page_text, req));
        }

        let truncated_text = if page_text.len() > 12000 {
            let mut end = 12000;
            while end > 0 && !page_text.is_char_boundary(end) {
                end -= 1;
            }
            &page_text[..end]
        } else {
            page_text
        };

        let prompt = format!(
            r#"You are an industrial supplier extraction specialist.
Analyze this crawled web page and extract the company details, product offers, and verbatim evidence quotes.

CRITICAL EVIDENCE RULES:
1. Every important field extracted MUST have a corresponding object in "evidence_items".
2. "evidence_quote" MUST BE A STRICT VERBATIM QUOTE (an exact substring) from the provided Page Text below.
3. If a field (e.g. price, moq, email, address) is NOT stated on the page, set it to null. DO NOT HALLUCINATE OR ESTIMATE!
4. Price amounts without published numbers MUST be null.
5. Country and address describe the SELLER physical business/contact location, never the product origin, brand headquarters, shipping destinations or search target. A China seller shipping to Indonesia is still in China. Return null if seller location is not established.
6. An Indonesian distributor selling imported Chinese products is in Indonesia only when its own Indonesian office/contact address is supported by the page.

Target product requirement:
{}

Page URL: {}
Page Title: {}
Page Text:
---
{}
---

Output JSON format:
{{
  "company_name": "string or null",
  "website": "string or null",
  "country_code": "2-letter uppercase or null",
  "country_name": "string or null",
  "address": "string or null",
  "supplier_type": "manufacturer | authorized_distributor | distributor | wholesaler | unknown",
  "business_category": "string or null",
  "public_email": "string or null",
  "public_phone": "string or null",
  "contact_page_url": "string or null",
  "offers": [
    {{
      "product_name": "string",
      "brand": "string or null",
      "model": "string or null",
      "part_number": "string or null",
      "description": "string or null",
      "attributes": {{}},
      "price_amount": number or null,
      "currency": "IDR | USD | EUR or null",
      "price_basis": "unit | pack | lot | unknown",
      "moq": "string or null",
      "lead_time": "string or null",
      "availability_text": "string or null"
    }}
  ],
  "evidence_items": [
    {{
      "field_path": "e.g. supplier.company_name or offer.price_amount",
      "field_value": "extracted value",
      "evidence_quote": "exact quote verbatim from the text",
      "confidence": 0.95
    }}
  ]
}}
"#,
            serde_json::to_string(req)?,
            page_url,
            page_title,
            truncated_text
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

        let resp = match self.client.post(&url).json(&req_body).send().await {
            Ok(r) if r.status().is_success() => r,
            other => {
                let reason = match other {
                    Ok(r) => format!("HTTP {}", r.status()),
                    Err(e) => e.to_string(),
                };
                tracing::warn!("Gemini extraction unavailable ({}), using heuristic fallback", reason);
                return Ok(self.fallback_extract_supplier(page_url, page_title, page_text, req));
            }
        };

        let resp_json: serde_json::Value = resp.json().await?;
        let text = resp_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("{}");

        match serde_json::from_str::<ExtractedSupplierPayload>(text) {
            Ok(mut payload) => {
                // Validate that evidence_quote actually appears in page_text
                payload.evidence_items.retain(|ev| {
                    !ev.evidence_quote.is_empty() && page_text.contains(&ev.evidence_quote)
                });
                let supported = |field: &str, value: &str| {
                    payload.evidence_items.iter().any(|ev| {
                        ev.field_path.contains(field)
                            && ev
                                .evidence_quote
                                .to_lowercase()
                                .contains(&value.to_lowercase())
                    })
                };
                if payload.public_email.as_deref().is_some_and(|v| {
                    let clean = v.replace(' ', "");
                    !supported("email", v) && !supported("email", &clean)
                }) {
                    payload.public_email = None;
                } else if let Some(ref em) = payload.public_email {
                    if em.contains(' ') {
                        payload.public_email = extract_valid_emails(&em.replace(' ', "")).into_iter().next();
                    }
                }
                if payload.public_phone.as_deref().is_some_and(|v| {
                    let vl = v.to_lowercase();
                    vl.contains("xxx") || vl.contains('*') || !supported("phone", v)
                }) {
                    payload.public_phone = None;
                }
                if payload
                    .address
                    .as_deref()
                    .is_some_and(|v| !supported("address", v))
                {
                    payload.address = None;
                }
                if payload
                    .country_name
                    .as_deref()
                    .is_some_and(|v| !supported("country", v))
                {
                    payload.country_name = None;
                    payload.country_code = None;
                }
                if payload.country_name.is_none() {
                    payload.country_code = None;
                }
                payload.offers.retain(|offer| {
                    let mut found = req.clone();
                    found.product_name = offer.product_name.clone();
                    found.part_number = offer.part_number.clone();
                    found.synonyms.clear();
                    product_mentioned_on_page(&found, page_text)
                });
                let offer_count = payload.offers.len();
                for offer in &mut payload.offers {
                    if offer.brand.as_deref().is_some_and(|brand| {
                        !location::contains_ascii_word(
                            &page_text.to_lowercase(),
                            &brand.to_lowercase(),
                        )
                    }) {
                        offer.brand = None;
                    }
                    if let Some(attributes) = offer.attributes.as_object_mut() {
                        attributes.retain(|key, value| {
                            payload
                                .evidence_items
                                .iter()
                                .any(|e| e.field_path.contains(key) && e.field_value == *value)
                        });
                    }
                    if !payload.evidence_items.iter().any(|e| {
                        e.field_path.contains("price")
                            && e.field_value.as_f64() == offer.price_amount
                            && offer.price_amount.is_some()
                            && (offer_count == 1
                                || e.evidence_quote
                                    .to_lowercase()
                                    .contains(&offer.product_name.to_lowercase())
                                || offer.part_number.as_deref().is_some_and(|pn| {
                                    e.evidence_quote.to_lowercase().contains(&pn.to_lowercase())
                                }))
                    }) {
                        offer.price_amount = None;
                        offer.currency = None;
                    }
                }
                Ok(payload)
            }
            _ => Ok(self.fallback_extract_supplier(page_url, page_title, page_text, req)),
        }
    }

    pub fn fallback_extract_supplier(
        &self,
        page_url: &str,
        page_title: &str,
        page_text: &str,
        req: &ProductRequirement,
    ) -> ExtractedSupplierPayload {
        let domain = extract_domain(page_url).unwrap_or_default();

        let mut company_name = None;
        let mut evidence_items = Vec::new();

        if !page_title.is_empty() {
            let parts: Vec<&str> = page_title
                .split(&['|', '-', '—', '–', '•'][..])
                .map(|p| p.trim())
                .filter(|p| {
                    let pl = p.to_lowercase();
                    !p.is_empty()
                        && ![
                            "indonetwork",
                            "indotrading",
                            "ralali",
                            "tokopedia",
                            "shopee",
                            "bukalapak",
                            "blibli",
                            "ensun",
                            "alibaba",
                            "made-in-china",
                        ]
                        .contains(&pl.as_str())
                })
                .collect();

            // 1. Prefer segment with explicit company legal form or industrial identity
            for part in &parts {
                let pl = part.to_lowercase();
                if pl.starts_with("pt ")
                    || pl.starts_with("pt.")
                    || pl.starts_with("cv ")
                    || pl.starts_with("cv.")
                    || pl.starts_with("ud ")
                    || pl.starts_with("ud.")
                    || pl.contains("co., ltd")
                    || pl.contains("co.,ltd")
                    || pl.contains("ltd")
                    || pl.contains("inc")
                    || pl.contains("corp")
                    || pl.contains("gmbh")
                    || pl.contains("pabrik")
                    || pl.contains("teknik")
                    || pl.contains("perkasa")
                    || pl.contains("industrial")
                {
                    company_name = Some(part.to_string());
                    break;
                }
            }

            // 2. If first segment is a product listing (e.g. "Jual SKF 6205 | Asia Global Teknik"), prefer the last brand segment
            if company_name.is_none() && parts.len() >= 2 {
                let first_l = parts[0].to_lowercase();
                if first_l.starts_with("jual")
                    || first_l.starts_with("buy")
                    || first_l.starts_with("harga")
                    || first_l.starts_with("beli")
                    || first_l.starts_with("item")
                    || first_l.contains("product")
                    || req
                        .part_number
                        .as_deref()
                        .map(|pn| first_l.contains(&pn.to_lowercase()))
                        .unwrap_or(false)
                {
                    if let Some(last) = parts.last() {
                        if last.len() >= 3 && last.len() <= 80 {
                            company_name = Some(last.to_string());
                        }
                    }
                }
            }

            if company_name.is_none() {
                // Check body text for an explicit PT/CV/Co., Ltd name before falling back to generic page title
                if let Some(body_co) = extract_legal_company_from_text(page_text) {
                    company_name = Some(body_co);
                } else if let Some(first) = parts.first() {
                    let fl = first.to_lowercase();
                    let is_noisy_seo = fl.starts_with("jual ")
                        || fl.starts_with("harga ")
                        || fl.starts_with("beli ")
                        || fl.starts_with("distributor ")
                        || fl.starts_with("supplier ");
                    if !is_noisy_seo && first.len() >= 3 && first.len() <= 80 {
                        company_name = Some(first.to_string());
                    }
                }
            }
        } else if let Some(body_co) = extract_legal_company_from_text(page_text) {
            company_name = Some(body_co);
        }

        if company_name.is_none() && !domain.is_empty() {
            let brand = domain.split('.').next().unwrap_or(&domain);
            let clean_brand: String = brand
                .split('-')
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            company_name = Some(clean_brand);
        }

        if let Some(ref cname) = company_name {
            if page_text.contains(cname.as_str()) || page_title.contains(cname.as_str()) {
                evidence_items.push(ExtractedEvidencePayload {
                    field_path: "supplier.company_name".to_string(),
                    field_value: serde_json::json!(cname),
                    evidence_quote: cname.clone(),
                    confidence: 0.85,
                });
            }
        }

        let emails = extract_valid_emails(page_text);
        let email = emails.into_iter().next();
        if let Some(em) = &email {
            evidence_items.push(ExtractedEvidencePayload {
                field_path: "supplier.public_email".to_string(),
                field_value: serde_json::json!(em),
                evidence_quote: em.clone(),
                confidence: 0.95,
            });
        }

        let phones = extract_phones_and_whatsapp(page_text);
        let phone = phones
            .into_iter()
            .find(|p| !p.to_lowercase().contains("xxx") && !p.contains('*'));
        if let Some(ph) = &phone {
            evidence_items.push(ExtractedEvidencePayload {
                field_path: "supplier.public_phone".to_string(),
                field_value: serde_json::json!(ph),
                evidence_quote: ph.clone(),
                confidence: 0.9,
            });
        }

        // Determine country from domain or verbatim page text markers
        let page_lower = page_text.to_lowercase();
        let mut country_code = None;
        let mut country_name = None;
        if domain.ends_with(".id") || domain.ends_with(".co.id") {
            country_code = Some("ID".to_string());
            country_name = Some("Indonesia".to_string());
        } else if domain.ends_with(".sg") || domain.ends_with(".com.sg") {
            country_code = Some("SG".to_string());
            country_name = Some("Singapore".to_string());
        } else if domain.ends_with(".cn") || domain.ends_with(".com.cn") {
            country_code = Some("CN".to_string());
            country_name = Some("China".to_string());
        } else if domain.ends_with(".my") || domain.ends_with(".com.my") {
            country_code = Some("MY".to_string());
            country_name = Some("Malaysia".to_string());
        } else if domain.ends_with(".tw") || domain.ends_with(".com.tw") {
            country_code = Some("TW".to_string());
            country_name = Some("Taiwan".to_string());
        } else if domain.ends_with(".kr") || domain.ends_with(".co.kr") {
            country_code = Some("KR".to_string());
            country_name = Some("South Korea".to_string());
        } else if domain.ends_with(".de") {
            country_code = Some("DE".to_string());
            country_name = Some("Germany".to_string());
        } else if domain.ends_with(".se") {
            country_code = Some("SE".to_string());
            country_name = Some("Sweden".to_string());
        } else if domain.ends_with(".jp") || domain.ends_with(".co.jp") {
            country_code = Some("JP".to_string());
            country_name = Some("Japan".to_string());
        } else if domain.ends_with(".co.uk") {
            country_code = Some("GB".to_string());
            country_name = Some("United Kingdom".to_string());
        } else {
            let markers = [
                ("indonesia", "ID", "Indonesia"),
                ("jabodetabek", "ID", "Indonesia"),
                ("jakarta", "ID", "Indonesia"),
                ("surabaya", "ID", "Indonesia"),
                ("tangerang", "ID", "Indonesia"),
                ("cikarang", "ID", "Indonesia"),
                ("bekasi", "ID", "Indonesia"),
                ("karawang", "ID", "Indonesia"),
                ("bandung", "ID", "Indonesia"),
                ("semarang", "ID", "Indonesia"),
                ("kudus", "ID", "Indonesia"),
                ("medan", "ID", "Indonesia"),
                ("makassar", "ID", "Indonesia"),
                ("gresik", "ID", "Indonesia"),
                ("sidoarjo", "ID", "Indonesia"),
                ("bogor", "ID", "Indonesia"),
                ("depok", "ID", "Indonesia"),
                ("batam", "ID", "Indonesia"),
                ("palembang", "ID", "Indonesia"),
                ("jawa barat", "ID", "Indonesia"),
                ("jawa timur", "ID", "Indonesia"),
                ("jawa tengah", "ID", "Indonesia"),
                ("singapore", "SG", "Singapore"),
                ("malaysia", "MY", "Malaysia"),
                ("kuala lumpur", "MY", "Malaysia"),
                ("penang", "MY", "Malaysia"),
                ("selangor", "MY", "Malaysia"),
                ("shenzhen", "CN", "China"),
                ("guangzhou", "CN", "China"),
                ("shanghai", "CN", "China"),
                ("beijing", "CN", "China"),
                ("ningbo", "CN", "China"),
                ("dongguan", "CN", "China"),
                ("suzhou", "CN", "China"),
                ("yiwu", "CN", "China"),
                ("hangzhou", "CN", "China"),
                ("china", "CN", "China"),
                ("taiwan", "TW", "Taiwan"),
                ("taipei", "TW", "Taiwan"),
                ("south korea", "KR", "South Korea"),
                ("seoul", "KR", "South Korea"),
                ("japan", "JP", "Japan"),
                ("tokyo", "JP", "Japan"),
                ("osaka", "JP", "Japan"),
                ("germany", "DE", "Germany"),
                ("munich", "DE", "Germany"),
                ("united states", "US", "United States"),
            ];
            let contact_text = location::contact_location_text(page_text);
            let contact_lower = contact_text.to_lowercase();
            for (kw, cc, cn) in markers {
                if location::contains_ascii_word(&contact_lower, kw) {
                    country_code = Some(cc.to_string());
                    country_name = Some(cn.to_string());
                    if !contact_text.is_empty() {
                        let quote = contact_text.clone();
                        evidence_items.push(ExtractedEvidencePayload {
                            field_path: "supplier.country_name".to_string(),
                            field_value: serde_json::json!(cn),
                            evidence_quote: quote,
                            confidence: 0.8,
                        });
                    }
                    break;
                }
            }
        }

        // Detect supplier type
        let supplier_type = if page_lower.contains("authorized distributor")
            || page_lower.contains("distributor resmi")
        {
            Some("authorized_distributor".to_string())
        } else if page_lower.contains("manufacturer")
            || page_lower.contains("pabrik")
            || page_lower.contains("produsen")
        {
            Some("manufacturer".to_string())
        } else if page_lower.contains("distributor")
            || page_lower.contains("supplier")
            || page_lower.contains("grosir")
        {
            Some("distributor".to_string())
        } else {
            Some("unknown".to_string())
        };

        // Extract verbatim product match evidence window
        let target_term = req
            .part_number
            .as_deref()
            .unwrap_or(&req.product_name)
            .to_lowercase();
        if let Some(idx) = page_lower.find(&target_term) {
            if let Some(quote) = extract_quote_window(page_text, idx, 140) {
                evidence_items.push(ExtractedEvidencePayload {
                    field_path: "offer.product_match".to_string(),
                    field_value: serde_json::json!(req.product_name),
                    evidence_quote: quote,
                    confidence: 0.85,
                });
            }
        }

        if !evidence_items
            .iter()
            .any(|e| e.field_path == "offer.product_match")
        {
            if let Some(pn) = req.part_number.as_deref() {
                let pieces: Vec<String> = pn
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|s| !s.is_empty())
                    .map(regex::escape)
                    .collect();
                if !pieces.is_empty() {
                    let pattern = format!(
                        r"(?i)(?:^|[^\pL\pN]){}(?:$|[^\pL\pN])",
                        pieces.join(r"[^\pL\pN]*")
                    );
                    if let Some(matched) = regex::Regex::new(&pattern).ok().and_then(|re| {
                        re.find(page_text)
                            .map(|m| (m.start(), m.as_str().to_string()))
                    }) {
                        evidence_items.push(ExtractedEvidencePayload {
                            field_path: "offer.part_number".into(),
                            field_value: serde_json::json!(pn),
                            evidence_quote: matched.1,
                            confidence: 0.85,
                        });
                    }
                }
            } else if product_mentioned_on_page(req, page_text) {
                'outer: for tok in product_meaningful_tokens(&req.product_name) {
                    for variant in expand_bilingual_token(&tok) {
                        if let Some(idx) = page_lower.find(variant) {
                            if let Some(quote) = extract_quote_window(page_text, idx, 140) {
                                evidence_items.push(ExtractedEvidencePayload {
                                    field_path: "offer.product_match".to_string(),
                                    field_value: serde_json::json!(req.product_name),
                                    evidence_quote: quote,
                                    confidence: 0.78,
                                });
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }

        let offer = ExtractedOfferPayload {
            product_name: req.product_name.clone(),
            brand: req
                .brand
                .as_ref()
                .filter(|brand| {
                    location::contains_ascii_word(&page_text.to_lowercase(), &brand.to_lowercase())
                })
                .cloned(),
            model: None,
            part_number: req.part_number.clone(),
            description: Some(page_title.to_string()),
            attributes: serde_json::json!({}),
            price_amount: None,
            currency: None,
            price_basis: Some("unknown".to_string()),
            moq: None,
            lead_time: None,
            availability_text: None,
        };

        let mut payload = ExtractedSupplierPayload {
            company_name,
            website: Some(page_url.to_string()),
            country_code,
            country_name,
            address: None,
            supplier_type,
            business_category: Some(req.category.clone()),
            public_email: email,
            public_phone: phone,
            contact_page_url: None,
            offers: if product_mentioned_on_page(req, page_text) {
                vec![offer]
            } else {
                vec![]
            },
            evidence_items,
        };
        location::enrich_extracted_location(&mut payload, page_url, page_text);
        payload
    }
}

fn is_indonesia_region(region: &str) -> bool {
    location::country_code_from_name(region) == Some("ID")
}

fn compact_search_subject(raw: &str) -> String {
    let stop_prefixes = [
        "cari ",
        "carikan ",
        "tolong cari ",
        "butuh ",
        "mencari ",
        "find ",
        "search ",
    ];
    let mut cleaned = raw.trim().to_string();
    let lower = cleaned.to_lowercase();
    for pref in stop_prefixes {
        if lower.starts_with(pref) {
            cleaned = cleaned[pref.len()..].trim().to_string();
            break;
        }
    }
    let words: Vec<&str> = cleaned.split_whitespace().take(8).collect();
    if words.is_empty() {
        raw.trim().to_string()
    } else {
        words.join(" ")
    }
}

fn extract_legal_company_from_text(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let prefix_re = regex::Regex::new(
        r"\b((?:PT|CV|UD)\.?\s+[A-Z][A-Za-z0-9&.,' -]{2,50}(?:\s+(?:Tbk|Indonesia|Mandiri|Jaya|Teknik|Perkasa|Abadi|Sentosa|Sejahtera|Utama|Nusantara|Industri))?)",
    )
    .ok()?;
    let limit = if text.len() > 2500 {
        let mut end = 2500;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    } else {
        text
    };
    if let Some(caps) = prefix_re.captures(limit) {
        if let Some(m) = caps.get(1) {
            let candidate = m
                .as_str()
                .trim()
                .trim_end_matches(&['.', ',', ';', '-'][..]);
            if candidate.len() >= 5 && candidate.len() <= 70 {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn extract_quote_window(text: &str, byte_idx: usize, max_len: usize) -> Option<String> {
    if text.is_empty() || byte_idx >= text.len() {
        return None;
    }
    let half = max_len / 2;
    let mut start = byte_idx.saturating_sub(half);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (byte_idx + half).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    let snippet = text[start..end].trim();
    if snippet.is_empty() {
        None
    } else {
        Some(snippet.to_string())
    }
}

pub fn product_meaningful_tokens(name: &str) -> Vec<String> {
    name.split(|c: char| !c.is_alphanumeric())
        .map(|s| s.to_lowercase())
        .filter(|s| {
            s.len() > 1
                && ![
                    "cari",
                    "supplier",
                    "vendor",
                    "jual",
                    "butuh",
                    "distributor",
                    "produk",
                    "buy",
                    "purchase",
                    "untuk",
                    "buat",
                    "dengan",
                    "yang",
                    "dan",
                    "atau",
                    "di",
                    "ke",
                    "dari",
                    "for",
                    "with",
                    "the",
                    "and",
                    "or",
                    "of",
                    "in",
                ]
                .contains(&s.as_str())
        })
        .collect()
}

pub fn expand_bilingual_token<'a>(token: &'a str) -> Vec<&'a str> {
    match token {
        "paper" | "kertas" => vec!["paper", "kertas"],
        "matte" | "matt" | "doff" => vec!["matte", "matt", "doff"],
        "barrier" | "barruer" => vec!["barrier", "laminasi", "metallized", "metalized", "foil"],
        "plastic" | "plastik" => vec!["plastic", "plastik"],
        "bottle" | "botol" => vec!["bottle", "botol"],
        "cap" | "caps" | "closure" | "tutup" => vec!["cap", "caps", "closure", "tutup"],
        "cup" | "cups" | "gelas" => vec!["cup", "cups", "gelas"],
        "packaging" | "kemasan" | "packing" => vec!["packaging", "kemasan", "packing"],
        "box" | "dus" | "kardus" | "kotak" => vec!["box", "dus", "kardus", "kotak"],
        "steel" | "baja" => vec!["steel", "baja"],
        "cable" | "kabel" => vec!["cable", "kabel"],
        "valve" | "katup" => vec!["valve", "katup"],
        other => vec![other],
    }
}

/// Sizes such as "1200x1000", "25mm" or "2inch" describe the product, they are not part numbers.
fn is_measurement_token(word: &str) -> bool {
    let w = word.to_lowercase();
    let dimension = regex::Regex::new(r"^\d+([.,]\d+)?([x×*]\d+([.,]\d+)?)+[a-z]*$");
    let with_unit = regex::Regex::new(
        r#"^\d+([.,]\d+)?(mm|cm|m|in|inch|"|gsm|ml|l|lt|liter|kg|g|gr|mic|micron|pcs|w|watt|v|volt|bar|psi|ton)$"#,
    );
    dimension.is_ok_and(|re| re.is_match(&w)) || with_unit.is_ok_and(|re| re.is_match(&w))
}

/// Splits a part number at separators and letter/digit boundaries ("6205-2RS" -> 6205, 2, RS)
/// so it still matches when a page writes it with different spacing or punctuation.
fn part_number_pieces(pn: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut prev_is_digit: Option<bool> = None;
    for ch in pn.chars() {
        if !ch.is_alphanumeric() {
            if !current.is_empty() {
                pieces.push(std::mem::take(&mut current));
            }
            prev_is_digit = None;
            continue;
        }
        let is_digit = ch.is_numeric();
        if prev_is_digit.is_some_and(|p| p != is_digit) && !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
        }
        current.push(ch);
        prev_is_digit = Some(is_digit);
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

/// Only public page content can establish a product offer; the search request cannot.
pub fn product_mentioned_on_page(req: &ProductRequirement, text: &str) -> bool {
    if let Some(pn) = req.part_number.as_deref().filter(|s| !s.trim().is_empty()) {
        let pieces: Vec<String> = part_number_pieces(pn)
            .iter()
            .map(|p| regex::escape(p))
            .collect();
        if pieces.is_empty() {
            return false;
        }
        let pattern = format!(
            r"(?i)(?:^|[^\pL\pN]){}(?:$|[^\pL\pN])",
            pieces.join(r"[^\pL\pN]*")
        );
        return regex::Regex::new(&pattern).is_ok_and(|re| re.is_match(text));
    }
    let lower = text.to_lowercase();
    std::iter::once(&req.product_name)
        .chain(req.synonyms.iter())
        .any(|name| {
            let tokens = product_meaningful_tokens(name);
            if tokens.is_empty() {
                return false;
            }
            let matched = tokens
                .iter()
                .filter(|t| {
                    expand_bilingual_token(t)
                        .iter()
                        .any(|v| crate::services::location::contains_ascii_word(&lower, v))
                })
                .count();
            let required = tokens.len().min(2);
            matched >= required
        })
}
