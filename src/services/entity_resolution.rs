use crate::domain::Supplier;
use crate::services::location;
use crate::utils::text::normalize_company_name;
use crate::utils::url_safety::extract_domain;

pub struct EntityResolution;

impl EntityResolution {
    pub fn normalize_supplier(
        raw_company_name: Option<&str>,
        raw_website: Option<&str>,
        raw_country: Option<&str>,
        raw_email: Option<&str>,
        raw_phone: Option<&str>,
        raw_type: Option<&str>,
        raw_category: Option<&str>,
    ) -> Supplier {
        let website = raw_website.map(|w| w.trim().to_string());
        let normalized_domain = website.as_deref().and_then(extract_domain);

        let canonical_name = if let Some(name) = raw_company_name {
            name.trim().to_string()
        } else if let Some(ref d) = normalized_domain {
            d.split('.').next().unwrap_or(d).to_uppercase()
        } else {
            "Unknown Supplier".to_string()
        };

        let normalized_name = normalize_company_name(&canonical_name);

        let mut country_name = raw_country
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty());
        let country_code = country_name
            .as_deref()
            .and_then(location::country_code_from_name)
            .or_else(|| normalized_domain.as_deref().and_then(location::country_code_from_domain))
            .or_else(|| raw_phone.and_then(location::country_code_from_phone))
            .map(|c| c.to_string());

        if country_name.is_none() {
            if let Some(ref cc) = country_code {
                country_name = Some(location::canonical_country_name(cc).to_string());
            }
        }

        let supplier_type = raw_type
            .map(|t| t.trim().to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());

        Supplier {
            id: 0,
            canonical_name,
            normalized_name,
            website,
            normalized_domain,
            country_code,
            country_name,
            address: None,
            supplier_type: Some(supplier_type),
            business_category: raw_category.map(|c| c.trim().to_string()),
            public_email: raw_email.map(|e| e.trim().to_lowercase()),
            public_phone: raw_phone.map(|p| p.trim().to_string()),
            contact_page_url: None,
            status: "discovered".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}


pub fn is_shared_supplier_platform(domain: &str) -> bool {
    [
        "indotrading.com",
        "indonetwork.co.id",
        "ralali.com",
        "ensun.io",
        "tokopedia.com",
        "alibaba.com",
        "made-in-china.com",
        "indiamart.com",
        "tradeindia.com",
        "ec21.com",
        "europages.com",
        "shopee.co.id",
        "bukalapak.com",
        "lazada.co.id",
        "blibli.com",
        "amazon.com",
        "ebay.com",
    ]
    .iter()
    .any(|host| domain == *host || domain.ends_with(&format!(".{}", host)))
}
