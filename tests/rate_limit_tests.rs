//! Rate Limiting Middleware Integration Tests
//!
//! Validates:
//! - Requests under the rate limit are allowed with 200 OK
//! - Requests exceeding the rate limit receive 429 Too Many Requests
//! - Retry-After header and JSON error body are properly returned

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

use rustbridge::api::{create_router_with_rate_limit, ApiState};
use rustbridge::config::AuthConfig;
use rustbridge::modbus::reader::RegisterStore;

fn disabled_auth() -> AuthConfig {
    AuthConfig {
        enabled: false,
        api_keys: vec![],
        exclude_paths: vec!["/health".to_string()],
    }
}

#[tokio::test]
async fn test_rate_limiter_allows_under_limit() {
    let register_store: RegisterStore = Arc::new(RwLock::new(HashMap::new()));
    let (write_tx, _write_rx) = tokio::sync::mpsc::channel(100);
    let state = ApiState::new(register_store, write_tx);

    // Limit: 10 requests per minute
    let app = create_router_with_rate_limit(state, disabled_auth(), true, 10);

    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn test_rate_limiter_enforces_limit_with_429() {
    let register_store: RegisterStore = Arc::new(RwLock::new(HashMap::new()));
    let (write_tx, _write_rx) = tokio::sync::mpsc::channel(100);
    let state = ApiState::new(register_store, write_tx);

    // Limit: 3 requests per minute
    let app = create_router_with_rate_limit(state, disabled_auth(), true, 3);

    // Send 3 requests (all allowed)
    for _ in 0..3 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    // 4th request must be rejected with 429
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        resp.headers()
            .get("Retry-After")
            .map(|v| v.to_str().unwrap()),
        Some("60")
    );

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "rate_limit_exceeded");
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("rate limit exceeded"));
}
