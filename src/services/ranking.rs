use crate::models::{ScoreBreakdown, SearchPreferences, Supplier, SupplierProduct};

pub struct RankingService;

impl RankingService {
    pub fn calculate_score(
        supplier: &Supplier,
        products: &[SupplierProduct],
        user_request: &str,
        important_specs: &[String],
        preferences: &SearchPreferences,
    ) -> ScoreBreakdown {
        let mut notes = Vec::new();

        // 1. Product and specification match (Weight: 40%)
        let _req_lower = user_request.to_lowercase();
        let mut product_score: f64 = 0.0;

        let mut spec_matches = 0;
        for spec in important_specs {
            let s_lower = spec.to_lowercase();
            let matches_supplier = supplier.company_name.to_lowercase().contains(&s_lower)
                || supplier
                    .description
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&s_lower);

            let matches_product = products.iter().any(|p| {
                p.product_name.to_lowercase().contains(&s_lower)
                    || p.model_or_specification
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&s_lower)
                    || p.brand
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&s_lower)
            });

            if matches_supplier || matches_product {
                spec_matches += 1;
            }
        }

        if !important_specs.is_empty() {
            let spec_ratio = spec_matches as f64 / important_specs.len() as f64;
            product_score += spec_ratio * 30.0;
            if spec_matches > 0 {
                notes.push(format!(
                    "Matched {}/{} technical specifications",
                    spec_matches,
                    important_specs.len()
                ));
            }
        } else {
            product_score += 20.0;
        }

        if !products.is_empty() {
            product_score += 10.0;
            notes.push(format!("Catalog lists {} relevant product items", products.len()));
        }

        product_score = product_score.clamp(0.0, 40.0);

        // 2. Location / shipping match (Weight: 20%)
        let mut location_score: f64 = 15.0; // Default reasonable score if global
        if let Some(ref country) = supplier.country {
            let country_lower = country.to_lowercase();
            if preferences.countries.is_empty() {
                location_score = 20.0;
                notes.push(format!("Supplier country: {}", country));
            } else {
                let preferred = preferences
                    .countries
                    .iter()
                    .any(|c| c.to_lowercase() == country_lower);
                if preferred {
                    location_score = 20.0;
                    notes.push(format!("Located in preferred country: {}", country));
                } else {
                    location_score = 10.0;
                    notes.push(format!("Located in alternative country: {}", country));
                }
            }
        } else {
            notes.push("Supplier country not specified on page".to_string());
        }
        location_score = location_score.clamp(0.0, 20.0);

        // 3. Company completeness (Weight: 15%)
        let mut company_score: f64 = 0.0;
        if supplier.website.is_some() {
            company_score += 5.0;
        }
        if supplier.description.is_some() {
            company_score += 5.0;
        }
        if supplier.category.is_some() {
            company_score += 5.0;
        }
        company_score = company_score.clamp(0.0, 15.0);

        // 4. Contact availability (Weight: 10%)
        let mut contact_score: f64 = 0.0;
        if supplier.email.is_some() {
            contact_score += 5.0;
            notes.push(format!("Direct email found: {}", supplier.email.as_deref().unwrap()));
        }
        if supplier.phone.is_some() {
            contact_score += 5.0;
            notes.push(format!("Direct phone found: {}", supplier.phone.as_deref().unwrap()));
        }
        contact_score = contact_score.clamp(0.0, 10.0);

        // 5. Price information trace (Weight: 15%)
        let mut price_score: f64 = 0.0;
        let products_with_price = products.iter().filter(|p| p.price_amount.is_some()).count();
        if products_with_price > 0 {
            price_score = 15.0;
            notes.push(format!("Found explicit pricing for {} items", products_with_price));
        } else {
            notes.push("Pricing requires inquiry/quote".to_string());
        }

        let total_score = (product_score + location_score + company_score + contact_score + price_score)
            .clamp(0.0, 100.0);

        ScoreBreakdown {
            product_match_score: product_score,
            location_score,
            company_profile_score: company_score,
            contact_score,
            price_score,
            total_score,
            notes,
        }
    }
}
