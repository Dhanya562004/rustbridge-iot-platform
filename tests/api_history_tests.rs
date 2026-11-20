//! Telemetry History & Platform Stats API Integration Tests
//!
//! Verifies endpoints:
//! - /api/devices/:id/status
//! - /api/devices/:id/history (pagination, filtering, time ranges, error validation)
//! - /api/stats (aggregate metrics)

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use chrono::Utc;
use http_body_util::BodyExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

use rustbridge::api::{create_router, ApiState};
use rustbridge::config::AuthConfig;
use rustbridge::modbus::reader::RegisterStore;
use rustbridge::storage::{DeviceRecord, InMemoryStorage, StorageBackend, TelemetryRecord};

fn disabled_auth() -> AuthConfig {
    AuthConfig {
        enabled: false,
        api_keys: vec![],
        exclude_paths: vec!["/health".to_string(), "/metrics".to_string()],
    }
}

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::json!({}));
    (status, json)
}

#[tokio::test]
async fn test_device_status_endpoint() {
    let storage = Arc::new(InMemoryStorage::new(100));
    let dev = DeviceRecord::new("device-alpha", "Alpha Sensor", "tcp", 1000);
    storage.save_device(&dev).await.unwrap();
    storage
        .update_device_status("device-alpha", true, true, false)
        .await
        .unwrap();

    let register_store: RegisterStore = Arc::new(RwLock::new(HashMap::new()));
    let (write_tx, _write_rx) = tokio::sync::mpsc::channel(100);
    let state = ApiState::with_storage(register_store, StorageBackend::Memory(storage), write_tx);

    let app = create_router(state, disabled_auth());

    let (status, json) = get_json(app.clone(), "/api/devices/device-alpha/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["device_id"], "device-alpha");
    assert_eq!(json["is_connected"], true);
    assert_eq!(json["total_reads"], 1);
    assert_eq!(json["total_errors"], 0);

    // Test non-existing device status
    let (status, json) = get_json(app, "/api/devices/unknown-device/status").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn test_device_history_pagination_and_filtering() {
    let storage = Arc::new(InMemoryStorage::new(500));
    let dev = DeviceRecord::new("turbine-01", "Wind Turbine", "tcp", 2000);
    storage.save_device(&dev).await.unwrap();

    let base_time = Utc::now();
    for i in 0..15 {
        storage
            .save_telemetry(&TelemetryRecord {
                id: None,
                device_id: "turbine-01".to_string(),
                register_name: if i % 2 == 0 {
                    "rpm".to_string()
                } else {
                    "vibration".to_string()
                },
                value: 100.0 + (i as f64) * 2.5,
                raw: vec![1000 + i as u16],
                unit: Some("rpm".to_string()),
                timestamp: base_time + chrono::Duration::seconds(i as i64),
            })
            .await
            .unwrap();
    }

    let register_store: RegisterStore = Arc::new(RwLock::new(HashMap::new()));
    let (write_tx, _write_rx) = tokio::sync::mpsc::channel(100);
    let state = ApiState::with_storage(register_store, StorageBackend::Memory(storage), write_tx);

    let app = create_router(state, disabled_auth());

    // Test default pagination (all 15 records)
    let (status, json) = get_json(app.clone(), "/api/devices/turbine-01/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["device_id"], "turbine-01");
    assert_eq!(json["total"], 15);
    let records = json["records"].as_array().unwrap();
    assert_eq!(records.len(), 15);

    // Test limit and offset pagination
    let (status, json) = get_json(
        app.clone(),
        "/api/devices/turbine-01/history?limit=5&offset=5",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["total"], 15);
    assert_eq!(json["limit"], 5);
    assert_eq!(json["offset"], 5);
    let records = json["records"].as_array().unwrap();
    assert_eq!(records.len(), 5);

    // Test register filtering (rpm only: 8 records out of 15)
    let (status, json) =
        get_json(app.clone(), "/api/devices/turbine-01/history?register=rpm").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["total"], 8);
    let records = json["records"].as_array().unwrap();
    assert_eq!(records.len(), 8);
    for r in records {
        assert_eq!(r["register_name"], "rpm");
    }
}

#[tokio::test]
async fn test_device_history_validation_errors() {
    let storage = Arc::new(InMemoryStorage::new(100));
    let register_store: RegisterStore = Arc::new(RwLock::new(HashMap::new()));
    let (write_tx, _write_rx) = tokio::sync::mpsc::channel(100);
    let state = ApiState::with_storage(register_store, StorageBackend::Memory(storage), write_tx);

    let app = create_router(state, disabled_auth());

    // Limit exceeding maximum 1000
    let (status, json) = get_json(app.clone(), "/api/devices/dev1/history?limit=5000").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"].as_str().unwrap().contains("Invalid limit"));

    // Invalid timestamp format
    let (status, json) = get_json(app.clone(), "/api/devices/dev1/history?start=not-a-date").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("Invalid start timestamp"));
}

#[tokio::test]
async fn test_platform_stats_endpoint() {
    let storage = Arc::new(InMemoryStorage::new(100));
    let dev1 = DeviceRecord::new("dev-1", "Sensor 1", "tcp", 1000);
    let dev2 = DeviceRecord::new("dev-2", "Sensor 2", "rtu", 2000);
    storage.save_device(&dev1).await.unwrap();
    storage.save_device(&dev2).await.unwrap();
    storage
        .update_device_status("dev-1", true, true, false)
        .await
        .unwrap();

    let register_store: RegisterStore = Arc::new(RwLock::new(HashMap::new()));
    let (write_tx, _write_rx) = tokio::sync::mpsc::channel(100);
    let state = ApiState::with_storage(register_store, StorageBackend::Memory(storage), write_tx);

    let app = create_router(state, disabled_auth());

    let (status, json) = get_json(app, "/api/stats").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["total_devices"], 2);
    assert_eq!(json["connected_devices"], 1);
    assert_eq!(json["disconnected_devices"], 1);
    assert_eq!(json["storage_type"], "memory");
}
