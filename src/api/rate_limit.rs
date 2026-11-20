//! API Rate Limiting Middleware
//!
//! Provides in-memory sliding window rate limiting per IP or API key.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Shared state for rate limiting
#[derive(Clone)]
pub struct RateLimiter {
    enabled: bool,
    max_rpm: u32,
    clients: Arc<RwLock<HashMap<String, (u32, Instant)>>>,
}

impl RateLimiter {
    pub fn new(enabled: bool, max_rpm: u32) -> Self {
        Self {
            enabled,
            max_rpm: if max_rpm == 0 { 120 } else { max_rpm },
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Check and record an incoming request for client key
    pub async fn check_rate_limit(&self, client_key: &str) -> bool {
        if !self.enabled {
            return true;
        }

        let now = Instant::now();
        let window = Duration::from_secs(60);
        let mut clients = self.clients.write().await;

        let entry = clients
            .entry(client_key.to_string())
            .or_insert_with(|| (0, now));

        if now.duration_since(entry.1) > window {
            // Window reset
            *entry = (1, now);
            true
        } else if entry.0 < self.max_rpm {
            entry.0 += 1;
            true
        } else {
            false
        }
    }
}

#[derive(Serialize)]
struct RateLimitError {
    error: &'static str,
    code: u16,
    message: &'static str,
}

/// Rate limiting middleware function for Axum
pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiter>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !limiter.is_enabled() {
        return next.run(request).await;
    }

    // Determine client identifier: API key header or client IP
    let client_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            request
                .headers()
                .get("X-Forwarded-For")
                .and_then(|v| v.to_str().ok())
        })
        .unwrap_or("default-client");

    if limiter.check_rate_limit(client_key).await {
        next.run(request).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", "60")],
            Json(RateLimitError {
                error: "rate_limit_exceeded",
                code: StatusCode::TOO_MANY_REQUESTS.as_u16(),
                message: "Request rate limit exceeded. Please wait 60 seconds before retrying.",
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_under_limit() {
        let limiter = RateLimiter::new(true, 5);
        for _ in 0..5 {
            assert!(limiter.check_rate_limit("client-1").await);
        }
        // 6th request exceeds limit of 5
        assert!(!limiter.check_rate_limit("client-1").await);

        // Different client should still be allowed
        assert!(limiter.check_rate_limit("client-2").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_disabled() {
        let limiter = RateLimiter::new(false, 1);
        assert!(limiter.check_rate_limit("client-1").await);
        assert!(limiter.check_rate_limit("client-1").await);
    }
}
