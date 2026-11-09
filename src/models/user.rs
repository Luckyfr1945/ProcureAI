use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Purchasing,
}

impl Default for UserRole {
    fn default() -> Self {
        Self::Purchasing
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::Purchasing => write!(f, "purchasing"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub is_active: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserDto {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserResponseDto {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: String,
}

impl From<User> for UserResponseDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            role: u.role,
            is_active: u.is_active == 1,
            created_at: u.created_at,
        }
    }
}

