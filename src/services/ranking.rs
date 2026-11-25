use crate::domain::{EvidenceItem, Product, ProductRequirement, Supplier, SupplierOffer};
use crate::services::location;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualScore {
    pub product_fit_score: f64,
    pub evidence_completeness_score: f64,
    pub notes: Vec<String>,
}

pub struct RankingService;

impl RankingService {
    /// Computes both Product Fit (0-100%) and Evidence Completeness (0-100%)
    pub fn calculate_scores(
        supplier: &Supplier,
        product: Option<&Product>,
        offer: Option<&SupplierOffer>,
        req: &ProductRequirement,
        evidence: &[EvidenceItem],
    ) -> DualScore {
        let mut notes = Vec::new();

        // ==========================================
        // 1. PRODUCT FIT SCORE (Weight 100%)
        // ==========================================
        let mut fit_score: f64 = 0.0;

        // 1.1 Part number / model match (35%)
        if let Some(target_pn) = &req.part_number {
            let tpn_lower = target_pn.to_lowercase();
            let mut matched_pn = false;
            if let Some(prod) = product {
                if let Some(pn) = &prod.part_number {
                    if pn.to_lowercase() == tpn_lower {
                        fit_score += 35.0;
                        matched_pn = true;
                        notes.push(format!("Exact part number match: {}", target_pn));
                    }
                }
            }
            if !matched_pn {
                if let Some(off) = offer {
                    if let Some(desc) = &off.description {
                        if desc.to_lowercase().contains(&tpn_lower) {
                            fit_score += 25.0;
                            matched_pn = true;
                            notes.push(format!(
                                "Part number mentioned in product description: {}",
                                target_pn
                            ));
                        }
                    }
                }
            }
            if !matched_pn {
                // Check supplier title / canonical name
                if supplier.canonical_name.to_lowercase().contains(&tpn_lower) {
                    fit_score += 20.0;
                    notes.push(format!(
                        "Supplier title references part number: {}",
                        target_pn
                    ));
                }
            }
        } else {
            // If no part number requested, reallocate 35% to product name / keywords match
            let req_name_lower = req.product_name.to_lowercase();
            let mut name_matched = false;
            if let Some(prod) = product {
                if prod.canonical_name.to_lowercase().contains(&req_name_lower) {
                    fit_score += 35.0;
                    name_matched = true;
                    notes.push("Product name matches query closely".to_string());
                }
            }
            if !name_matched {
                if let Some(off) = offer {
                    if let Some(desc) = &off.description {
                        if desc.to_lowercase().contains(&req_name_lower) {
                            fit_score += 30.0;
                            name_matched = true;
                            notes.push("Offer description matches query".to_string());
                        }
                    }
                }
            }
            if !name_matched {
                if supplier
                    .canonical_name
                    .to_lowercase()
                    .contains(&req_name_lower)
                {
                    fit_score += 25.0;
                    notes.push("Company name/title contains target product name".to_string());
                } else {
                    let prod_haystack = format!(
                        "{} {}",
                        product.map(|p| p.canonical_name.as_str()).unwrap_or(""),
                        offer.and_then(|o| o.description.as_deref()).unwrap_or("")
                    )
                    .to_lowercase();
                    let tokens =
                        crate::services::gemini::product_meaningful_tokens(&req.product_name);
                    let matched_count = tokens
                        .iter()
                        .filter(|t| {
                            crate::services::gemini::expand_bilingual_token(t)
                                .iter()
                                .any(|v| prod_haystack.contains(v))
                        })
                        .count();
                    let ratio = if tokens.is_empty() {
                        0.0
                    } else {
                        matched_count as f64 / tokens.len() as f64
                    };
                    fit_score += 15.0 + (ratio * 18.0);
                    if matched_count > 0 {
                        notes.push(format!(
                            "Matched {}/{} product keywords",
                            matched_count,
                            tokens.len()
                        ));
                    }
                }
            }
        }

        // 1.2 Required attributes match (30%)
        if !req.required_attributes.is_empty() {
            let mut matched_attrs = 0;
            if let Some(off) = offer {
                if let Some(attr_json) = &off.attributes_json {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(attr_json) {
                        for required in &req.required_attributes {
                            if attribute_matches(required, &val) {
                                matched_attrs += 1;
                            }
                        }
                    }
                }
            }
            let ratio = matched_attrs as f64 / req.required_attributes.len() as f64;
            let attr_points = ratio * 30.0;
            fit_score += attr_points;
            if matched_attrs > 0 {
                notes.push(format!(
                    "Matched {}/{} required technical attributes",
                    matched_attrs,
                    req.required_attributes.len()
                ));
            }
        } else {
            fit_score += 25.0;
        }

        // 1.3 Brand match (15%)
        if let Some(target_brand) = &req.brand {
            let tb_lower = target_brand.to_lowercase();
            let mut brand_matched = false;
            if let Some(prod) = product {
                if let Some(b) = &prod.brand {
                    if b.to_lowercase() == tb_lower {
                        fit_score += 15.0;
                        brand_matched = true;
                        notes.push(format!("Exact brand match: {}", target_brand));
                    }
                }
            }
            if !brand_matched {
                if supplier.canonical_name.to_lowercase().contains(&tb_lower) {
                    fit_score += 10.0;
                    notes.push(format!("Supplier title contains brand: {}", target_brand));
                }
            }
        } else {
            fit_score += 15.0;
        }

        // 1.4 Category match (10%)
        let target_cat = &req.category;
        if let Some(sup_cat) = &supplier.business_category {
            if sup_cat.to_lowercase() == target_cat.to_lowercase() {
                fit_score += 10.0;
            } else {
                fit_score += 5.0;
            }
        } else {
            fit_score += 5.0;
        }

        // 1.5 Supplier type match (10%)
        let sup_type = supplier.supplier_type.as_deref().unwrap_or("unknown");
        if req
            .supplier_types
            .iter()
            .any(|t| t.to_lowercase() == sup_type.to_lowercase())
        {
            fit_score += 10.0;
            notes.push(format!("Matches requested supplier type: {}", sup_type));
        } else if sup_type != "unknown" {
            fit_score += 5.0;
        }

        // 1.6 Target region alignment bonus / mismatch penalty (country + subregion/city precision)
        let target_codes = location::target_country_codes(&req.target_regions);
        let subregions = location::target_subregions(&req.target_regions);

        if !target_codes.is_empty() {
            let sup_country = supplier.country_name.as_deref().unwrap_or("");
            let sup_code = supplier
                .country_code
                .as_deref()
                .and_then(location::country_code_from_name)
                .or_else(|| location::country_code_from_name(sup_country))
                .or_else(|| {
                    supplier
                        .normalized_domain
                        .as_deref()
                        .and_then(location::country_code_from_domain)
                })
                .or_else(|| {
                    supplier
                        .public_phone
                        .as_deref()
                        .and_then(location::country_code_from_phone)
                });
            let sup_addr = supplier.address.as_deref().unwrap_or("").to_lowercase();
            let sup_name = supplier.canonical_name.to_lowercase();
            let ev_blob = evidence
                .iter()
                .map(|e| e.evidence_quote.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            let loc_blob = format!("{} {} {}", sup_addr, sup_name, ev_blob);

            let country_matched = sup_code
                .map(|sc| target_codes.iter().any(|tc| tc.eq_ignore_ascii_case(sc)))
                .unwrap_or(false);

            let subregion_matched = !subregions.is_empty()
                && subregions
                    .iter()
                    .any(|sr| location::contains_ascii_word(&loc_blob, sr));

            if !subregions.is_empty() {
                if country_matched && subregion_matched {
                    fit_score += 20.0;
                    notes.push(format!(
                        "Matches target city/region ({})",
                        req.target_regions.join(", ")
                    ));
                } else if country_matched {
                    // Check if supplier explicitly sits in another Indonesian city outside the target
                    let detected_other_city =
                        location::extract_indonesian_city_or_region(&sup_addr)
                            .map(|c| c.to_lowercase());
                    if let Some(ref other) = detected_other_city {
                        if !subregions.iter().any(|sr| other.contains(sr)) {
                            fit_score += 1.0;
                            notes.push(format!(
                                "In target country, but city '{}' differs from requested ({})",
                                other,
                                subregions.join(", ")
                            ));
                        } else {
                            fit_score += 20.0;
                        }
                    } else {
                        fit_score += 5.0;
                        notes.push("Matches target country (city unspecified)".to_string());
                    }
                } else if sup_code.is_some() {
                    fit_score -= 25.0;
                    notes.push(format!(
                        "Location '{}' differs from target region ({})",
                        supplier
                            .country_name
                            .as_deref()
                            .or(sup_code)
                            .unwrap_or("unknown"),
                        req.target_regions.join(", ")
                    ));
                }
            } else if country_matched {
                fit_score += 12.0;
                notes.push(format!(
                    "Matches target country ({})",
                    req.target_regions.join(", ")
                ));
            } else if sup_code.is_some() {
                fit_score -= 25.0;
                notes.push(format!(
                    "Location '{}' differs from target region ({})",
                    supplier
                        .country_name
                        .as_deref()
                        .or(sup_code)
                        .unwrap_or("unknown"),
                    req.target_regions.join(", ")
                ));
            }
        }

        let proven = product.is_some_and(|p| {
            crate::services::stored_search::matches_product(
                req,
                p,
                offer.and_then(|o| o.description.as_deref()),
            )
        }) && evidence.iter().any(|e| {
            (e.field_path.contains("product")
                || e.field_path.contains("part_number")
                || e.field_path.contains("attribute")
                || e.field_path.contains("description")
                || e.field_path.contains("offer"))
                && !e.evidence_quote.trim().is_empty()
        });
        let product_fit_score = if proven {
            fit_score.clamp(15.0, 100.0)
        } else {
            0.0
        };

        // ==========================================
        // 2. EVIDENCE COMPLETENESS SCORE (Weight 100%)
        // ==========================================
        let mut completeness_score: f64 = 0.0;

        // 2.1 Company website verified with real evidence (20%)
        if supplier.website.is_some() {
            completeness_score += 20.0;
        }

        // 2.2 Contact evidence (email or phone backed by quote) (20%)
        let has_contact_evidence = evidence
            .iter()
            .any(|e| e.field_path.contains("email") || e.field_path.contains("phone"));
        if has_contact_evidence {
            completeness_score += 20.0;
            notes.push("Direct contact backed by verbatim page evidence".to_string());
        } else if supplier.public_email.is_some() || supplier.public_phone.is_some() {
            completeness_score += 10.0;
        }

        // 2.3 Country / Geographic location evidence (20%)
        if supplier.country_name.is_some() || supplier.country_code.is_some() {
            completeness_score += 20.0;
        }

        // 2.4 Product specification evidence (20%)
        let has_spec_evidence = evidence.iter().any(|e| {
            e.field_path.contains("attribute")
                || e.field_path.contains("part_number")
                || e.field_path.contains("product")
        });
        if has_spec_evidence {
            completeness_score += 20.0;
            notes.push("Product specifications backed by page evidence quotes".to_string());
        } else if offer.is_some() {
            completeness_score += 10.0;
        }

        // 2.5 Price / Quotation terms evidence (20%)
        let has_price_evidence = evidence.iter().any(|e| e.field_path.contains("price"));
        if has_price_evidence {
            completeness_score += 20.0;
            notes.push("Published pricing backed by source quotation/evidence".to_string());
        } else if let Some(off) = offer {
            if off.price_amount.is_some() && off.price_amount.unwrap_or(0.0) > 0.0 {
                completeness_score += 15.0;
            } else {
                notes.push("Harga: Tidak dipublikasikan / minta quotation".to_string());
                completeness_score += 5.0; // Partial score for explicit acknowledgment
            }
        } else {
            notes.push("Harga: Tidak dipublikasikan / minta quotation".to_string());
        }

        let evidence_completeness_score = completeness_score.clamp(0.0, 100.0);

        DualScore {
            product_fit_score,
            evidence_completeness_score,
            notes,
        }
    }
}

/// A named attribute alone is not proof that the requested value is satisfied.
fn attribute_matches(required: &str, attributes: &serde_json::Value) -> bool {
    fn tokens(text: &str) -> Vec<String> {
        let re = regex::Regex::new(r"[a-zA-Z]+|[0-9]+(?:[.,][0-9]+)?").unwrap();
        re.find_iter(&text.to_lowercase())
            .map(|m| match m.as_str() {
                "diameter" | "dia" => "diameter".into(),
                other => other.to_string(),
            })
            .collect()
    }
    let wanted = tokens(required);
    if wanted.len() < 2 || attributes.get(required).is_some() {
        return false;
    }
    attributes.as_object().is_some_and(|map| {
        map.iter().any(|(key, value)| {
            if value.is_null() {
                return false;
            }
            let candidate = tokens(&format!("{} {}", key.replace('_', " "), value));
            wanted.iter().all(|t| candidate.contains(t))
        })
    })
}
#[cfg(test)]
mod attribute_tests {
    use super::*;
    #[test]
    fn values_and_units_must_match_not_just_keys() {
        let data = serde_json::json!({"inner_diameter_mm":25, "certificate":"SNI"});
        assert!(attribute_matches("diameter 25mm", &data));
        assert!(!attribute_matches("diameter 50mm", &data));
        assert!(!attribute_matches("diameter 25cm", &data));
        assert!(!attribute_matches("inner_diameter_mm", &data));
    }
}
