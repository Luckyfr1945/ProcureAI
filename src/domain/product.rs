use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub id: i64,
    pub canonical_name: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub part_number: Option<String>,
    pub category_key: Option<String>,
    pub attributes_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SupplierOffer {
    pub id: i64,
    pub supplier_id: i64,
    pub product_id: i64,
    pub source_id: Option<i64>,
    pub product_url: Option<String>,
    pub description: Option<String>,
    pub attributes_json: Option<String>,
    pub price_amount: Option<f64>,
    pub currency: Option<String>,
    pub price_basis: Option<String>,
    pub moq: Option<String>,
    pub lead_time: Option<String>,
    pub availability_text: Option<String>,
    pub product_fit_score: f64,
    pub evidence_completeness_score: f64,
    pub extracted_at: String,
}
