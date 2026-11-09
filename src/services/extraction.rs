use crate::models::{Supplier, SupplierProduct};
use crate::providers::gemini::{ExtractedSupplierInfo, GeminiClient};
use crate::services::crawler::CrawlResult;
use crate::utils::text::normalize_company_name;
use crate::utils::url_safety::extract_domain;

pub struct ExtractionService {
    gemini: GeminiClient,
}

impl ExtractionService {
    pub fn new(gemini: GeminiClient) -> Self {
        Self { gemini }
    }

    pub async fn extract_candidate(
        &self,
        crawl: &CrawlResult,
    ) -> anyhow::Result<(Supplier, Vec<SupplierProduct>)> {
        let domain = extract_domain(&crawl.final_url).unwrap_or_else(|| "unknown.com".to_string());

        let extracted: ExtractedSupplierInfo = self
            .gemini
            .extract_supplier_info(&crawl.final_url, &crawl.page_title, &crawl.clean_text)
            .await?;

        let company_name = extracted
            .company_name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| {
                // Fallback to domain or title
                domain.clone()
            });

        let normalized_name = normalize_company_name(&company_name);
        let canonical_key = domain.clone();

        // Email from Gemini or HTML parser
        let mut email = extracted.email;
        if email.is_none() && !crawl.emails.is_empty() {
            email = Some(crawl.emails[0].clone());
        }

        // Phone from Gemini or HTML parser
        let mut phone = extracted.phone;
        if phone.is_none() && !crawl.phones.is_empty() {
            phone = Some(crawl.phones[0].clone());
        }

        let supplier = Supplier {
            id: 0,
            canonical_key,
            company_name,
            normalized_name,
            website: Some(crawl.final_url.clone()),
            domain: Some(domain),
            country: extracted.country,
            city: extracted.city,
            email,
            phone,
            category: extracted.category,
            description: extracted.description.or(crawl.meta_description.clone()),
            verification_status: "discovered".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let now = chrono::Utc::now().to_rfc3339();
        let products: Vec<SupplierProduct> = extracted
            .products
            .into_iter()
            .map(|p| SupplierProduct {
                id: 0,
                supplier_id: 0,
                source_id: None,
                product_name: p.product_name,
                brand: p.brand,
                model_or_specification: p.model_or_specification,
                price_amount: p.price_amount,
                price_currency: p.price_currency,
                minimum_order_quantity: p.minimum_order_quantity,
                availability_note: p.availability_note,
                product_url: Some(crawl.final_url.clone()),
                price_observed_at: Some(now.clone()),
            })
            .collect();

        Ok((supplier, products))
    }
}
