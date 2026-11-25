//! Reuse saved product offers, without treating a supplier's company name as product proof.
use crate::{
    database::repositories::{evidence, products, search_jobs, suppliers},
    domain::{Product, ProductRequirement},
    services::{RankingService, location},
};
use sqlx::SqlitePool;
use std::collections::HashMap;

fn compact(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| {
            match s {
                "bantalan" => "bearing",
                "katup" => "valve",
                _ => s,
            }
            .to_string()
        })
        .collect()
}
pub fn matches_product(
    req: &ProductRequirement,
    product: &Product,
    description: Option<&str>,
) -> bool {
    if let Some(brand) = req.brand.as_deref().filter(|s| !s.trim().is_empty()) {
        // A conflicting known brand excludes the product, even for identical model numbers.
        if product
            .brand
            .as_deref()
            .is_some_and(|b| compact(b) != compact(brand))
        {
            return false;
        }
    }
    if let Some(part) = req.part_number.as_deref().filter(|s| !s.trim().is_empty()) {
        return [product.part_number.as_deref(), product.model.as_deref()]
            .into_iter()
            .flatten()
            .any(|p| compact(p) == compact(part));
    }
    let blob = words(&format!(
        "{} {} {} {} {}",
        product.canonical_name,
        product.brand.as_deref().unwrap_or(""),
        product.model.as_deref().unwrap_or(""),
        product.part_number.as_deref().unwrap_or(""),
        description.unwrap_or("")
    ));
    std::iter::once(&req.product_name)
        .chain(req.synonyms.iter())
        .any(|term| {
            let tokens: Vec<_> = words(term)
                .into_iter()
                .filter(|w| {
                    ![
                        "cari",
                        "supplier",
                        "vendor",
                        "jual",
                        "distributor",
                        "butuh",
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
                    .contains(&w.as_str())
                })
                .collect();
            if tokens.is_empty() {
                return false;
            }
            let matched = tokens
                .iter()
                .filter(|t| {
                    crate::services::gemini::expand_bilingual_token(t)
                        .iter()
                        .any(|v| blob.iter().any(|b| b == v))
                })
                .count();
            let required = tokens.len().min(2);
            matched >= required
        })
}
pub async fn reuse_suppliers(
    pool: &SqlitePool,
    job: &str,
    req: &ProductRequirement,
) -> anyhow::Result<HashMap<i64, (f64, f64)>> {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT DISTINCT supplier_id FROM supplier_offers")
        .fetch_all(pool)
        .await?;
    let mut scores = HashMap::new();
    for id in ids {
        let Some(supplier) = suppliers::get_supplier_by_id(pool, id).await? else {
            continue;
        };
        if !location::consistent_supplier_location(
            supplier.country_code.as_deref(),
            supplier.country_name.as_deref(),
            supplier.address.as_deref(),
            supplier.public_phone.as_deref(),
        ) {
            continue;
        }
        if supplier.status == "rejected"
            || !location::supplier_matches_targets(
                supplier.country_code.as_deref(),
                supplier.country_name.as_deref(),
                supplier.address.as_deref(),
                &req.target_regions,
            )
        {
            continue;
        }
        let ev = evidence::get_evidence_by_supplier(pool, id).await?;
        let mut best: Option<(f64, f64)> = None;
        for offer in products::get_supplier_offers(pool, id).await? {
            let Some(product) = products::get_product_by_id(pool, offer.product_id).await? else {
                continue;
            };
            if !matches_product(req, &product, offer.description.as_deref()) {
                continue;
            }
            let score =
                RankingService::calculate_scores(&supplier, Some(&product), Some(&offer), req, &ev);
            let pair = (score.product_fit_score, score.evidence_completeness_score);
            if pair.0 <= 0.0 {
                continue;
            }
            if best.is_none_or(|b| pair > b) {
                best = Some(pair);
            }
        }
        if let Some(pair) = best {
            search_jobs::save_job_result(pool, job, id, pair.0, pair.1, 1).await?;
            scores.insert(id, pair);
        }
    }
    Ok(scores)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_identity_handles_formatting_but_rejects_other_models_and_brands() {
        let mut req: ProductRequirement = serde_json::from_value(serde_json::json!({
            "product_name": "bearing", "category": "bearing", "part_number": "6205-2RS", "brand": "SKF"
        })).unwrap();
        let mut product = Product {
            id: 1,
            canonical_name: "Bantalan SKF 6205 2RS".into(),
            brand: Some("SKF".into()),
            model: None,
            part_number: Some("6205 2RS".into()),
            category_key: None,
            attributes_json: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!(matches_product(&req, &product, None));
        product.part_number = Some("6305-2RS".into());
        assert!(!matches_product(&req, &product, None));
        product.part_number = Some("6205-2RS".into());
        product.brand = Some("NSK".into());
        assert!(!matches_product(&req, &product, None));
        req.part_number = None;
        req.brand = None;
        req.product_name = "cari vendor bearing".into();
        assert!(matches_product(&req, &product, None));
        req.product_name = "valve".into();
        assert!(!matches_product(&req, &product, None));
    }
}
