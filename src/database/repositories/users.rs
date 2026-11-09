use crate::models::{User, UserRole};
use sqlx::SqlitePool;

pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    password_hash: &str,
    role: UserRole,
) -> anyhow::Result<i64> {
    let role_str = role.to_string();
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO users (username, password_hash, role)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
    )
    .bind(username)
    .bind(password_hash)
    .bind(role_str)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_user_by_username(
    pool: &SqlitePool,
    username: &str,
) -> anyhow::Result<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, role, is_active, created_at
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

pub async fn get_user_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, role, is_active, created_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

pub async fn count_users(pool: &SqlitePool) -> anyhow::Result<i64> {
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    Ok(cnt)
}
