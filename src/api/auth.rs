use crate::auth::middleware::AuthenticatedUser;
use crate::auth::password::{hash_password, verify_password};
use crate::auth::session::create_session_token;
use crate::database::repositories::users;
use crate::models::{CreateUserDto, UserRole};
use crate::state::AppState;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct LoginDto {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponseDto {
    pub message: String,
    pub token: String,
    pub user: UserInfoDto,
}

#[derive(Debug, Serialize)]
pub struct UserInfoDto {
    pub id: i64,
    pub username: String,
    pub role: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(dto): Json<LoginDto>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if dto.username.len() > 200 || dto.password.len() > 4096 || dto.username.trim().is_empty() || dto.password.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "VALIDATION_ERROR",
                    "message": "Username and password are required"
                }
            })),
        ));
    }

    let rate_error = || (StatusCode::TOO_MANY_REQUESTS, Json(json!({"error":{"message":"Terlalu banyak percobaan masuk. Coba lagi setelah 15 menit."}})));
    let _slot = state.login_slots.try_acquire().map_err(|_| rate_error())?;
    let key = dto.username.trim().to_lowercase();
    {
        let mut attempts = state.login_attempts.lock().await;
        attempts.retain(|_, (_, at)| at.elapsed() < std::time::Duration::from_secs(900));
        if attempts.len() >= 10000 && !attempts.contains_key(&key) { return Err(rate_error()); }
        let entry = attempts.entry(key.clone()).or_insert((0, std::time::Instant::now()));
        if entry.0 >= 5 { return Err(rate_error()); }
        entry.0 += 1;
    }
    let user = users::get_user_by_username(&state.db, &dto.username)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "INTERNAL_ERROR",
                        "message": format!("Database error: {}", e)
                    }
                })),
            )
        })?;

    let user = match user {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "code": "INVALID_CREDENTIALS",
                        "message": "Invalid username or password"
                    }
                })),
            ));
        }
    };

    if user.is_active != 1 {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "code": "ACCOUNT_DISABLED",
                    "message": "Account is disabled"
                }
            })),
        ));
    }

    let password = dto.password.clone();
    let hash = user.password_hash.clone();
    let valid = tokio::task::spawn_blocking(move || verify_password(&password, &hash)).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":{"message":"Gagal memeriksa akun"}}))))?;
    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "code": "INVALID_CREDENTIALS",
                    "message": "Invalid username or password"
                }
            })),
        ));
    }

    state.login_attempts.lock().await.remove(&key);
    // 7 days token expiration
    let duration_secs = 7 * 24 * 3600;
    let token = create_session_token(
        user.id,
        &user.username,
        &user.role,
        &state.config.session_secret,
        duration_secs,
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "TOKEN_ERROR",
                    "message": format!("Failed to generate session token: {}", e)
                }
            })),
        )
    })?;

    let secure = if state.config.app_env == "production" {
        "; Secure"
    } else {
        ""
    };
    let cookie_val = format!(
        "session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        token, duration_secs, secure
    );

    let mut headers = HeaderMap::new();
    if let Ok(val) = HeaderValue::from_str(&cookie_val) {
        headers.insert(header::SET_COOKIE, val);
    }

    let resp_body = Json(json!({
        "message": "Login successful",
        "token": token,
        "user": {
            "id": user.id,
            "username": user.username,
            "role": user.role
        }
    }));

    Ok((StatusCode::OK, headers, resp_body))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if let Some(token) = crate::auth::middleware::request_token(&headers) {
        if let Ok(claims) =
            crate::auth::session::verify_session_token(&token, &state.config.session_secret)
        {
            sqlx::query(
                "INSERT OR IGNORE INTO revoked_sessions(token_hash,expires_at) VALUES (?,?)",
            )
            .bind(crate::auth::middleware::token_hash(&token))
            .bind(claims.exp)
            .execute(&state.db)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error":{"message":"Gagal mengakhiri sesi"}})),
                )
            })?;
        }
    }
    let mut headers = HeaderMap::new();
    let secure = if state.config.app_env == "production" {
        "; Secure"
    } else {
        ""
    };
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
            secure
        ))
        .unwrap(),
    );
    Ok((
        StatusCode::OK,
        headers,
        Json(json!({"message":"Logged out successfully"})),
    ))
}

pub async fn me(auth_user: AuthenticatedUser) -> impl IntoResponse {
    Json(json!({
        "user": {
            "id": auth_user.user_id,
            "username": auth_user.username,
            "role": auth_user.role
        }
    }))
}

pub async fn register_user(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(dto): Json<CreateUserDto>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if auth_user.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "code": "FORBIDDEN",
                    "message": "Only admins can create new users"
                }
            })),
        ));
    }

    if dto.username.len() > 200 || dto.password.len() > 4096 || dto.username.trim().is_empty() || dto.password.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "VALIDATION_ERROR",
                    "message": "Username and password are required"
                }
            })),
        ));
    }

    let password_hash = hash_password(&dto.password).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "CRYPTO_ERROR",
                    "message": format!("Password hashing failed: {}", e)
                }
            })),
        )
    })?;

    let role = match dto.role.as_deref() {
        Some("admin") => UserRole::Admin,
        _ => UserRole::Purchasing,
    };

    let user_id = users::create_user(&state.db, &dto.username, &password_hash, role.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": {
                        "code": "USERNAME_EXISTS",
                        "message": format!("Failed to create user (username may already exist): {}", e)
                    }
                })),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "User created successfully",
            "user": {
                "id": user_id,
                "username": dto.username,
                "role": role.to_string()
            }
        })),
    ))
}
