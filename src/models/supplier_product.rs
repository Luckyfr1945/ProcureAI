use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SupplierProduct {
    pub id: i64,
    pub supplier_id: i64,
    pub source_id: Option<i64>,
    pub product_name: String,
    pub brand: Option<String>,
    pub model_or_specification: Option<String>,
    pub price_amount: Option<f64>,
    pub price_currency: Option<String>,
    pub minimum_order_quantity: Option<String>,
    pub availability_note: Option<String>,
    pub product_url: Option<String>,
    pub price_observed_at: Option<String>,
}
