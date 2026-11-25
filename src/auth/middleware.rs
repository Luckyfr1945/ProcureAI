use crate::auth::session::verify_session_token;
use crate::state::AppState;
use axum::{
    Json,
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: i64,
    pub username: String,
    pub role: String,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let secret = &app_state.config.session_secret;

        // 1. Try Authorization header: Bearer <token>
        let mut token: Option<String> = None;
        if let Some(auth_val) = parts.headers.get(header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_val.to_str() {
                if let Some(t) = auth_str.strip_prefix("Bearer ") {
                    token = Some(t.trim().to_string());
                }
            }
        }

        // 2. Fallback to Cookie header: session=<token>
        if token.is_none() {
            if let Some(cookie_val) = parts.headers.get(header::COOKIE) {
                if let Ok(cookie_str) = cookie_val.to_str() {
                    for cookie in cookie_str.split(';') {
                        let trimmed = cookie.trim();
                        if let Some(val) = trimmed.strip_prefix("session=") {
                            token = Some(val.to_string());
                            break;
                        }
                    }
                }
            }
        }

        let token = match token {
            Some(t) => t,
            None => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "error": {
                            "code": "UNAUTHORIZED",
                            "message": "Authentication token missing"
                        }
                    })),
                )
                    .into_response());
            }
        };

        match verify_session_token(&token, secret) {
            Ok(claims) => {
                let user = crate::database::repositories::users::get_user_by_id(
                    &app_state.db,
                    claims.user_id,
                )
                .await
                .map_err(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Authentication database unavailable",
                    )
                        .into_response()
                })?;
                let revoked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM revoked_sessions WHERE token_hash = ? AND expires_at >= unixepoch()")
                    .bind(token_hash(&token)).fetch_one(&app_state.db).await
                    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Authentication database unavailable").into_response())?;
                match user {
                    Some(u) if u.is_active == 1 && revoked == 0 => Ok(AuthenticatedUser {
                        user_id: u.id,
                        username: u.username,
                        role: u.role,
                    }),
                    _ => Err((
                        StatusCode::UNAUTHORIZED,
                        "Session expired or account disabled",
                    )
                        .into_response()),
                }
            }
            Err(e) => Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "code": "UNAUTHORIZED",
                        "message": format!("Invalid or expired session: {}", e)
                    }
                })),
            )
                .into_response()),
        }
    }
}

pub struct RequireAdmin(pub AuthenticatedUser);

impl<S> FromRequestParts<S> for RequireAdmin
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        if user.role != "admin" {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": {
                        "code": "FORBIDDEN",
                        "message": "Administrator privileges required"
                    }
                })),
            )
                .into_response());
        }
        Ok(RequireAdmin(user))
    }
}

pub fn token_hash(token: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
pub fn request_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get(header::COOKIE)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| {
                    s.split(';')
                        .find_map(|c| c.trim().strip_prefix("session=").map(str::to_owned))
                })
        })
}
