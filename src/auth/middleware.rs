use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use crate::auth::session::verify_session_token;
use crate::state::AppState;

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
            Ok(claims) => Ok(AuthenticatedUser {
                user_id: claims.user_id,
                username: claims.username,
                role: claims.role,
            }),
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
