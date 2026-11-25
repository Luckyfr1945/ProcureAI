use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Supplier {
    pub id: i64,
    pub canonical_name: String,
    pub normalized_name: String,
    pub website: Option<String>,
    pub normalized_domain: Option<String>,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub address: Option<String>,
    pub supplier_type: Option<String>,
    pub business_category: Option<String>,
    pub public_email: Option<String>,
    pub public_phone: Option<String>,
    pub contact_page_url: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SupplierSource {
    pub id: i64,
    pub supplier_id: i64,
    pub url: String,
    pub canonical_url: String,
    pub page_type: Option<String>,
    pub page_title: Option<String>,
    pub content_hash: Option<String>,
    pub retrieved_at: String,
    pub http_status: Option<i64>,
    pub source_type: Option<String>,
}
