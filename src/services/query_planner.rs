use crate::domain::ProductRequirement;
use crate::services::gemini::{GeminiClient, PlannedQuery};
use crate::services::location;

pub struct QueryPlanner {
    gemini: GeminiClient,
}

impl QueryPlanner {
    pub fn new(gemini: GeminiClient) -> Self {
        Self { gemini }
    }

    pub async fn generate_search_plan(
        &self,
        req: &ProductRequirement,
        max_queries: usize,
    ) -> anyhow::Result<Vec<PlannedQuery>> {
        let mut queries = self
            .gemini
            .plan_queries(req, max_queries)
            .await
            .unwrap_or_else(|_| self.gemini.fallback_plan_queries(req, max_queries));
        if max_queries >= 8 {
            queries.truncate(max_queries / 2);
        }
        queries.extend(self.gemini.fallback_plan_queries(req, max_queries));

        // Deduplicate queries
        let mut seen = std::collections::HashSet::new();
        queries.retain(|q| seen.insert(q.query_text.to_lowercase()));

        if queries.is_empty() {
            queries = self.gemini.fallback_plan_queries(req, max_queries);
        }

        let mut queries = location::enforce_query_regions(queries, req, max_queries);
        let mut seen = std::collections::HashSet::new();
        queries.retain(|q| seen.insert(q.query_text.to_lowercase()));
        Ok(queries)
    }

    /// Generates targeted queries specifically designed to address information gaps
    pub fn generate_gap_queries(
        &self,
        req: &ProductRequirement,
        gaps: &[String],
        max_queries: usize,
    ) -> Vec<PlannedQuery> {
        let mut planned = Vec::new();
        let name = &req.product_name;

        let target_codes = location::target_country_codes(&req.target_regions);
        let subregions = location::target_subregions(&req.target_regions);
        let is_id_target = target_codes.iter().any(|c| c == "ID");
        let region_hint = location::primary_subregion_for_query(&subregions).or_else(|| {
            target_codes
                .first()
                .map(|c| location::canonical_country_name(c).to_string())
        });
        let region_suffix = region_hint
            .as_deref()
            .map(|r| {
                if is_id_target && !r.eq_ignore_ascii_case("indonesia") {
                    format!(" {} Indonesia", r)
                } else {
                    format!(" {}", r)
                }
            })
            .unwrap_or_default();

        for gap in gaps {
            match gap.as_str() {
                "pricing" => {
                    if let Some(pn) = &req.part_number {
                        let q = if is_id_target {
                            format!("{} harga satuan distributor{}", pn, region_suffix)
                        } else {
                            format!("{} price quote supplier{}", pn, region_suffix)
                        };
                        planned.push(PlannedQuery {
                            query_text: q,
                            query_type: "gap_targeted".to_string(),
                            language: if is_id_target { "id" } else { "en" }.to_string(),
                            rationale: "Targeting missing pricing data".to_string(),
                        });
                    } else {
                        let q = if is_id_target {
                            format!("{} harga grosir distributor{}", name, region_suffix)
                        } else {
                            format!("{} wholesale price quote{}", name, region_suffix)
                        };
                        planned.push(PlannedQuery {
                            query_text: q,
                            query_type: "gap_targeted".to_string(),
                            language: if is_id_target { "id" } else { "en" }.to_string(),
                            rationale: "Targeting missing pricing data".to_string(),
                        });
                    }
                }
                "specifications" => {
                    if let Some(pn) = &req.part_number {
                        planned.push(PlannedQuery {
                            query_text: format!(
                                "{} datasheet catalog specification{}",
                                pn, region_suffix
                            ),
                            query_type: "gap_targeted".to_string(),
                            language: "en".to_string(),
                            rationale: "Targeting missing technical specification details"
                                .to_string(),
                        });
                    }
                }
                "contact" => {
                    let brand_str = req.brand.as_deref().unwrap_or(name);
                    planned.push(PlannedQuery {
                        query_text: format!(
                            "{} distributor{} contact email whatsapp telephone",
                            brand_str, region_suffix
                        ),
                        query_type: "gap_targeted".to_string(),
                        language: "en".to_string(),
                        rationale: "Targeting missing verified supplier contact information"
                            .to_string(),
                    });
                }
                _ => {}
            }
        }

        location::enforce_query_regions(planned, req, max_queries)
    }
}
