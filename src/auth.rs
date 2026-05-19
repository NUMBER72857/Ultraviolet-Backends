//! Merchant authentication and session enforcement.
//!
//! This module exists because invoice routes must never trust a caller-supplied
//! merchant id. Login creates opaque bearer tokens stored as hashes in
//! PostgreSQL, and each protected handler resolves the merchant from that token.

use axum::{http::HeaderMap, Json};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};

use crate::{error::ApiError, prefixed_id};

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub user_id: String,
    pub merchant_id: String,
    pub role: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub token_type: &'static str,
    pub expires_at: DateTime<Utc>,
    pub merchant_id: String,
    pub role: String,
}

#[derive(FromRow)]
struct MerchantUserForLogin {
    id: String,
    merchant_id: String,
    password_hash: String,
    role: String,
}

#[derive(FromRow)]
struct SessionUser {
    user_id: String,
    merchant_id: String,
    role: String,
}

pub async fn login(
    pool: &PgPool,
    session_secret: &str,
    session_ttl_hours: i64,
    payload: LoginRequest,
) -> Result<Json<LoginResponse>, ApiError> {
    let email = payload.email.trim().to_ascii_lowercase();
    if email.is_empty() || payload.password.is_empty() {
        return Err(ApiError::BadRequest("email and password are required"));
    }

    let user = sqlx::query_as::<_, MerchantUserForLogin>(
        r#"
        SELECT merchant_users.id, merchant_users.merchant_id, merchant_users.password_hash, merchant_users.role
        FROM merchant_users
        JOIN merchants ON merchants.id = merchant_users.merchant_id
        WHERE lower(merchant_users.email) = $1
          AND merchant_users.status = 'active'
          AND merchants.status = 'active'
        "#,
    )
    .bind(&email)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::Unauthorized("invalid email or password"))?;

    if !verify_password(&payload.password, &user.password_hash) {
        return Err(ApiError::Unauthorized("invalid email or password"));
    }

    let token = new_session_token();
    let token_hash = hash_session_token(session_secret, &token);
    let expires_at = Utc::now() + Duration::hours(session_ttl_hours);

    sqlx::query(
        r#"
        INSERT INTO merchant_sessions (id, merchant_user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(prefixed_id("sess"))
    .bind(&user.id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer",
        expires_at,
        merchant_id: user.merchant_id,
        role: user.role,
    }))
}

pub async fn logout(
    pool: &PgPool,
    session_secret: &str,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let token = bearer_token(headers)?;
    let token_hash = hash_session_token(session_secret, token);
    sqlx::query(
        r#"
        UPDATE merchant_sessions
        SET revoked_at = now(), updated_at = now()
        WHERE token_hash = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(token_hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn require_auth(
    pool: &PgPool,
    session_secret: &str,
    headers: &HeaderMap,
) -> Result<AuthContext, ApiError> {
    let token = bearer_token(headers)?;
    let token_hash = hash_session_token(session_secret, token);

    let session = sqlx::query_as::<_, SessionUser>(
        r#"
        SELECT
          merchant_users.id AS user_id,
          merchant_users.merchant_id,
          merchant_users.role
        FROM merchant_sessions
        JOIN merchant_users ON merchant_users.id = merchant_sessions.merchant_user_id
        JOIN merchants ON merchants.id = merchant_users.merchant_id
        WHERE merchant_sessions.token_hash = $1
          AND merchant_sessions.revoked_at IS NULL
          AND merchant_sessions.expires_at > now()
          AND merchant_users.status = 'active'
          AND merchants.status = 'active'
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::Unauthorized("invalid or expired session"))?;

    sqlx::query("UPDATE merchant_sessions SET last_seen_at = now(), updated_at = now() WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await?;

    Ok(AuthContext {
        user_id: session.user_id,
        merchant_id: session.merchant_id,
        role: session.role,
    })
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Unauthorized(
            "Authorization bearer token is required",
        ))?;

    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(ApiError::Unauthorized(
            "Authorization bearer token is required",
        ))
}

fn new_session_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn hash_session_token(session_secret: &str, token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_secret.as_bytes());
    hasher.update(b":");
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn verify_password(password: &str, stored_hash: &str) -> bool {
    if let Some(rest) = stored_hash.strip_prefix("pbkdf2_sha256$") {
        let mut parts = rest.split('$');
        let iterations = match parts.next().and_then(|value| value.parse::<u32>().ok()) {
            Some(value) if value >= 100_000 => value,
            _ => return false,
        };
        let salt = match parts.next().and_then(|value| hex::decode(value).ok()) {
            Some(value) if !value.is_empty() => value,
            _ => return false,
        };
        let expected = match parts.next().and_then(|value| hex::decode(value).ok()) {
            Some(value) if !value.is_empty() => value,
            _ => return false,
        };
        if parts.next().is_some() {
            return false;
        }

        return constant_time_eq(
            &pbkdf2_sha256(password.as_bytes(), &salt, iterations, expected.len()),
            &expected,
        );
    }

    // Legacy bootstrap-only format. Existing deployments should migrate these
    // hashes immediately after the first authenticated login.
    if let Some(expected) = stored_hash.strip_prefix("sha256$") {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        return constant_time_eq(
            hex::encode(hasher.finalize()).as_bytes(),
            expected.as_bytes(),
        );
    }

    false
}

fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32, output_len: usize) -> Vec<u8> {
    type HmacSha256 = Hmac<Sha256>;

    let mut output = Vec::with_capacity(output_len);
    let mut block_index = 1_u32;

    while output.len() < output_len {
        let mut mac = HmacSha256::new_from_slice(password).expect("HMAC accepts any key length");
        mac.update(salt);
        mac.update(&block_index.to_be_bytes());
        let mut u = mac.finalize().into_bytes().to_vec();
        let mut block = u.clone();

        for _ in 1..iterations {
            let mut mac =
                HmacSha256::new_from_slice(password).expect("HMAC accepts any key length");
            mac.update(&u);
            u = mac.finalize().into_bytes().to_vec();
            for (left, right) in block.iter_mut().zip(&u) {
                *left ^= *right;
            }
        }

        output.extend_from_slice(&block);
        block_index = block_index.checked_add(1).expect("PBKDF2 block overflow");
    }

    output.truncate(output_len);
    output
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    left.iter()
        .zip(right)
        .fold(0_u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

#[cfg(test)]
mod tests {
    use super::{pbkdf2_sha256, verify_password};
    use sha2::{Digest, Sha256};

    #[test]
    fn verifies_pbkdf2_sha256_password_hashes() {
        let salt = b"merchant-specific-salt";
        let hash = pbkdf2_sha256(b"correct horse battery staple", salt, 100_000, 32);
        let salt_hex = hex::encode(salt);
        let hash_hex = hex::encode(hash);
        let stored = format!("pbkdf2_sha256$100000${salt_hex}${hash_hex}");

        assert!(verify_password("correct horse battery staple", &stored));
        assert!(!verify_password("wrong", &stored));
    }

    #[test]
    fn verifies_sha256_password_hashes() {
        let mut hasher = Sha256::new();
        hasher.update(b"correct horse battery staple");
        let stored = format!("sha256${}", hex::encode(hasher.finalize()));

        assert!(verify_password("correct horse battery staple", &stored));
        assert!(!verify_password("wrong", &stored));
    }

    #[test]
    fn rejects_unknown_password_hash_formats() {
        assert!(!verify_password("password", "password"));
    }
}
