use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClaims {
    pub user_id: i64,
    pub username: String,
    pub role: String,
    pub exp: i64,
}

pub fn create_session_token(
    user_id: i64,
    username: &str,
    role: &str,
    secret: &str,
    duration_secs: i64,
) -> anyhow::Result<String> {
    let exp = chrono::Utc::now().timestamp() + duration_secs;
    let claims = SessionClaims {
        user_id,
        username: username.to_string(),
        role: role.to_string(),
        exp,
    };

    let payload_bytes = serde_json::to_vec(&claims)?;
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_bytes);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| anyhow::anyhow!("HMAC key error: {}", e))?;
    mac.update(payload_b64.as_bytes());
    let sig = mac.finalize().into_bytes();
    let sig_b64 = URL_SAFE_NO_PAD.encode(sig);

    Ok(format!("{}.{}", payload_b64, sig_b64))
}

pub fn verify_session_token(token: &str, secret: &str) -> anyhow::Result<SessionClaims> {
    let mut parts = token.split('.');
    let payload_b64 = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid token format"))?;
    let sig_b64 = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid token format"))?;

    if parts.next().is_some() {
        return Err(anyhow::anyhow!("Invalid token format"));
    }

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| anyhow::anyhow!("HMAC key error: {}", e))?;
    mac.update(payload_b64.as_bytes());

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| anyhow::anyhow!("Invalid signature base64"))?;
    mac.verify_slice(&sig_bytes)
        .map_err(|_| anyhow::anyhow!("Invalid token signature"))?;

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| anyhow::anyhow!("Invalid payload base64"))?;
    let claims: SessionClaims = serde_json::from_slice(&payload_bytes)?;

    let now = chrono::Utc::now().timestamp();
    if claims.exp < now {
        return Err(anyhow::anyhow!("Token has expired"));
    }

    Ok(claims)
}
