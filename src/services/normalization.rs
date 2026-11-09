use crate::models::Supplier;
use crate::utils::text::normalize_company_name;
use crate::utils::url_safety::extract_domain;

pub fn normalize_supplier(mut supplier: Supplier) -> Supplier {
    supplier.company_name = supplier.company_name.trim().to_string();
    supplier.normalized_name = normalize_company_name(&supplier.company_name);

    if let Some(ref w) = supplier.website {
        if supplier.domain.is_none() {
            supplier.domain = extract_domain(w);
        }
    }

    // Set canonical key from domain if available, else normalized name + country
    if let Some(ref domain) = supplier.domain {
        supplier.canonical_key = domain.to_lowercase();
    } else {
        let country_part = supplier
            .country
            .as_deref()
            .unwrap_or("global")
            .to_lowercase();
        supplier.canonical_key = format!("{}_{}", supplier.normalized_name, country_part);
    }

    supplier
}

