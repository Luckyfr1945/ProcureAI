use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRequirement {
    pub product_name: String,
    pub category: String,
    pub brand: Option<String>,
    pub part_number: Option<String>,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub required_attributes: Vec<String>,
    #[serde(default)]
    pub optional_attributes: Vec<String>,
    #[serde(default)]
    pub supplier_types: Vec<String>,
    #[serde(default)]
    pub target_regions: Vec<String>,
    #[serde(default)]
    pub search_languages: Vec<String>,
    #[serde(default)]
    pub pr_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSearchJobRequest {
    #[serde(alias = "request")]
    pub input_text: String,
    #[serde(default)]
    pub target_regions: Vec<String>,
    #[serde(default)]
    pub preferred_languages: Vec<String>,
    #[serde(default)]
    pub required_attributes: Vec<String>,
    #[serde(default)]
    pub pr_number: Option<String>,
    pub max_duration_minutes: Option<u64>,
    pub max_search_requests: Option<u32>,
}
