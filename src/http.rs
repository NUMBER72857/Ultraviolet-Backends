//! HTTP edge hardening for the Axum service.
//!
//! The backend is small, but it still needs boring production controls:
//! bounded JSON bodies, a restrictive CORS policy, and a cheap per-client rate
//! limiter before requests reach money-moving handlers.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer};

use crate::{error::ApiError, AppState};

#[derive(Debug)]
pub struct RateLimiter {
    max_requests: u32,
    window: Duration,
    buckets: Mutex<HashMap<String, Bucket>>,
}

#[derive(Clone, Copy, Debug)]
struct Bucket {
    started_at: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn new(max_requests: u32) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(60),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn check(&self, key: String) -> bool {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().expect("rate limiter lock poisoned");
        buckets.retain(|_, bucket| now.duration_since(bucket.started_at) < self.window);

        let bucket = buckets.entry(key).or_insert(Bucket {
            started_at: now,
            count: 0,
        });

        if now.duration_since(bucket.started_at) >= self.window {
            *bucket = Bucket {
                started_at: now,
                count: 0,
            };
        }

        bucket.count += 1;
        bucket.count <= self.max_requests
    }
}

pub fn body_limit_layer(max_bytes: usize) -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(max_bytes)
}

pub fn cors_layer(allowed_origin: &str) -> Result<CorsLayer, String> {
    if allowed_origin == "*" {
        return Ok(CorsLayer::permissive());
    }

    let origin = allowed_origin
        .parse::<HeaderValue>()
        .map_err(|_| "CORS_ALLOWED_ORIGIN must be '*' or a valid origin".to_string())?;

    Ok(CorsLayer::new()
        .allow_origin(origin)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("idempotency-key"),
        ]))
}

pub async fn rate_limit(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let key = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            request
                .extensions()
                .get::<axum::extract::ConnectInfo<SocketAddr>>()
                .map(|connect_info| connect_info.0.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    if !state.rate_limiter.check(key) {
        return Err(ApiError::TooManyRequests("rate limit exceeded"));
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::RateLimiter;

    #[test]
    fn blocks_after_limit() {
        let limiter = RateLimiter::new(2);
        assert!(limiter.check("client".to_string()));
        assert!(limiter.check("client".to_string()));
        assert!(!limiter.check("client".to_string()));
    }
}
