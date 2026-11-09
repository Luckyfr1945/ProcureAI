use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VerificationStatus {
    Unverified,
    Discovered,
    Extracted,
    Verified,
    Contacted,
    Approved,
}

impl std::fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unverified => write!(f, "unverified"),
            Self::Discovered => write!(f, "discovered"),
            Self::Extracted => write!(f, "extracted"),
            Self::Verified => write!(f, "verified"),
            Self::Contacted => write!(f, "contacted"),
            Self::Approved => write!(f, "approved"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Supplier {
    pub id: i64,
    pub canonical_key: String,
    pub company_name: String,
    pub normalized_name: String,
    pub website: Option<String>,
    pub domain: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub verification_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSupplierDto {
    pub company_name: Option<String>,
    pub website: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub verification_status: Option<String>,
}

