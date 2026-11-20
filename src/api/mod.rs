//! REST API module with WebSocket support, telemetry history, and metrics
//!
//! Provides REST endpoints for reading/writing Modbus registers,
//! querying historical telemetry, monitoring device health status,
//! and WebSocket streaming for real-time register updates.

pub mod auth;
pub mod rate_limit;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use metrics_exporter_prometheus::PrometheusHandle;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

use crate::config::AuthConfig;
use crate::metrics;
use crate::modbus::reader::RegisterStore;
use crate::storage::{InMemoryStorage, SharedStorage, StorageBackend, TelemetryQuery};

use self::auth::{api_key_auth, AuthState};
use self::rate_limit::{rate_limit_middleware, RateLimiter};

/// Broadcast channel capacity for WebSocket updates
const BROADCAST_CAPACITY: usize = 1024;

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub register_store: RegisterStore,
    pub storage: SharedStorage,
    pub update_tx: broadcast::Sender<RegisterUpdate>,
    pub write_tx: tokio::sync::mpsc::Sender<WriteRequest>,
    pub metrics_handle: Option<PrometheusHandle>,
    pub start_time: Instant,
}

impl ApiState {
    /// Create new API state with broadcast channel and storage
    #[allow(dead_code)]
    pub fn new(
        register_store: RegisterStore,
        write_tx: tokio::sync::mpsc::Sender<WriteRequest>,
    ) -> Self {
        let (update_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            register_store,
            storage: StorageBackend::Memory(Arc::new(InMemoryStorage::default())),
            update_tx,
            write_tx,
            metrics_handle: None,
            start_time: Instant::now(),
        }
    }

    /// Create new API state with storage
    pub fn with_storage(
        register_store: RegisterStore,
        storage: SharedStorage,
        write_tx: tokio::sync::mpsc::Sender<WriteRequest>,
    ) -> Self {
        let (update_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            register_store,
            storage,
            update_tx,
            write_tx,
            metrics_handle: None,
            start_time: Instant::now(),
        }
    }

    /// Create new API state with metrics handle and storage
    pub fn with_metrics(
        register_store: RegisterStore,
        storage: SharedStorage,
        write_tx: tokio::sync::mpsc::Sender<WriteRequest>,
        metrics_handle: PrometheusHandle,
    ) -> Self {
        let (update_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            register_store,
            storage,
            update_tx,
            write_tx,
            metrics_handle: Some(metrics_handle),
            start_time: Instant::now(),
        }
    }

    /// Get a receiver for register updates
    pub fn subscribe(&self) -> broadcast::Receiver<RegisterUpdate> {
        self.update_tx.subscribe()
    }
}

/// Register update message for WebSocket broadcast
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterUpdate {
    pub device_id: String,
    pub register_name: String,
    pub value: f64,
    pub raw: Vec<u16>,
    pub unit: Option<String>,
    pub timestamp: String,
}

/// Write request sent to Modbus client
#[derive(Debug)]
pub struct WriteRequest {
    pub device_id: String,
    pub address: u16,
    pub value: u16,
    pub response_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
}

/// Create the API router
#[allow(dead_code)]
pub fn create_router(state: ApiState, auth_config: AuthConfig) -> Router {
    create_router_with_rate_limit(state, auth_config, false, 120)
}

/// Create router with rate limit configuration
pub fn create_router_with_rate_limit(
    state: ApiState,
    auth_config: AuthConfig,
    rate_limit_enabled: bool,
    rate_limit_rpm: u32,
) -> Router {
    let auth_state = Arc::new(AuthState::new(auth_config));
    let rate_limiter = Arc::new(RateLimiter::new(rate_limit_enabled, rate_limit_rpm));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health & Stats
        .route("/health", get(health))
        .route("/api/info", get(api_info))
        .route("/api/stats", get(api_stats))
        // Metrics (Prometheus)
        .route("/metrics", get(metrics_handler))
        // Devices
        .route("/api/devices", get(list_devices))
        .route("/api/devices/:device_id", get(get_device))
        .route("/api/devices/:device_id/status", get(get_device_status))
        // Telemetry History
        .route("/api/devices/:device_id/history", get(get_device_history))
        // Registers (read)
        .route("/api/devices/:device_id/registers", get(get_registers))
        .route(
            "/api/devices/:device_id/registers/:register_name",
            get(get_register),
        )
        // Registers (write)
        .route(
            "/api/devices/:device_id/registers/:register_name",
            post(write_register),
        )
        // WebSocket
        .route("/ws", get(ws_handler))
        // Layer: Rate limiting
        .layer(middleware::from_fn_with_state(
            rate_limiter,
            rate_limit_middleware,
        ))
        // Layer: API key authentication
        .layer(middleware::from_fn_with_state(auth_state, api_key_auth))
        // Layer: CORS
        .layer(cors)
        .with_state(Arc::new(state))
}

// ============================================================================
// Error Handling
// ============================================================================

/// API error response
#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ApiError {
    pub fn new(code: StatusCode, error: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            code,
            Json(Self {
                error: error.into(),
                code: code.as_u16(),
                details: None,
            }),
        )
    }

    pub fn with_details(
        code: StatusCode,
        error: impl Into<String>,
        details: impl Into<String>,
    ) -> (StatusCode, Json<Self>) {
        (
            code,
            Json(Self {
                error: error.into(),
                code: code.as_u16(),
                details: Some(details.into()),
            }),
        )
    }
}

// ============================================================================
// Health, Info & Stats Endpoints
// ============================================================================

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
    pub storage_type: &'static str,
    pub devices_total: usize,
    pub devices_connected: usize,
}

async fn health(State(state): State<Arc<ApiState>>) -> Json<HealthResponse> {
    let start = Instant::now();
    let uptime_seconds = state.start_time.elapsed().as_secs();

    let store = state.register_store.read().await;
    let devices_total = store.len();

    // Check device records in storage
    let (total_storage_devices, connected_devices) = match state.storage.list_devices().await {
        Ok(devs) => {
            let connected = devs.iter().filter(|d| d.is_connected).count();
            (devs.len(), connected)
        }
        Err(_) => (devices_total, 0),
    };

    let total = if total_storage_devices > 0 {
        total_storage_devices
    } else {
        devices_total
    };

    let _ = state.storage.health_check().await;

    metrics::record_api_request(
        "GET",
        "/health",
        StatusCode::OK.as_u16(),
        start.elapsed().as_secs_f64(),
    );

    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds,
        storage_type: state.storage.storage_type(),
        devices_total: total,
        devices_connected: connected_devices,
    })
}

/// Global platform statistics response
#[derive(Serialize)]
pub struct PlatformStatsResponse {
    pub uptime_seconds: u64,
    pub total_devices: usize,
    pub connected_devices: usize,
    pub disconnected_devices: usize,
    pub total_telemetry_records: usize,
    pub storage_type: &'static str,
    pub metrics_enabled: bool,
}

async fn api_stats(State(state): State<Arc<ApiState>>) -> Json<PlatformStatsResponse> {
    let start = Instant::now();
    let uptime_seconds = state.start_time.elapsed().as_secs();

    let devices = state.storage.list_devices().await.unwrap_or_default();
    let total_devices = devices.len();
    let connected_devices = devices.iter().filter(|d| d.is_connected).count();
    let disconnected_devices = total_devices.saturating_sub(connected_devices);
    let total_telemetry_records = state.storage.total_telemetry_count().await.unwrap_or(0);

    metrics::record_api_request(
        "GET",
        "/api/stats",
        StatusCode::OK.as_u16(),
        start.elapsed().as_secs_f64(),
    );

    Json(PlatformStatsResponse {
        uptime_seconds,
        total_devices,
        connected_devices,
        disconnected_devices,
        total_telemetry_records,
        storage_type: state.storage.storage_type(),
        metrics_enabled: state.metrics_handle.is_some(),
    })
}

/// API info response
#[derive(Serialize)]
pub struct ApiInfoResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub maintainer: &'static str,
    pub repository: &'static str,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Serialize)]
pub struct EndpointInfo {
    pub method: &'static str,
    pub path: &'static str,
    pub description: &'static str,
}

async fn api_info(State(_state): State<Arc<ApiState>>) -> Json<ApiInfoResponse> {
    let start = Instant::now();
    let resp = Json(ApiInfoResponse {
        name: "RustBridge IoT Platform API",
        version: env!("CARGO_PKG_VERSION"),
        description: "Industrial IoT Protocol Bridge - Modbus TCP/RTU to JSON/MQTT Gateway",
        maintainer: "Dhanya",
        repository: "https://github.com/Dhanya562004/rustbridge-iot-platform.git",
        endpoints: vec![
            EndpointInfo {
                method: "GET",
                path: "/health",
                description: "Health check and uptime status",
            },
            EndpointInfo {
                method: "GET",
                path: "/api/info",
                description: "Platform API catalog and metadata",
            },
            EndpointInfo {
                method: "GET",
                path: "/api/stats",
                description: "Global bridge telemetry and device stats",
            },
            EndpointInfo {
                method: "GET",
                path: "/metrics",
                description: "Prometheus scraping endpoint",
            },
            EndpointInfo {
                method: "GET",
                path: "/api/devices",
                description: "List all configured devices",
            },
            EndpointInfo {
                method: "GET",
                path: "/api/devices/:device_id",
                description: "Get device details and current values",
            },
            EndpointInfo {
                method: "GET",
                path: "/api/devices/:device_id/status",
                description: "Get device live connection and retry health status",
            },
            EndpointInfo {
                method: "GET",
                path: "/api/devices/:device_id/history",
                description: "Query historical telemetry with pagination and filtering",
            },
            EndpointInfo {
                method: "GET",
                path: "/api/devices/:device_id/registers",
                description: "List all registers for a device",
            },
            EndpointInfo {
                method: "GET",
                path: "/api/devices/:device_id/registers/:name",
                description: "Get specific register value",
            },
            EndpointInfo {
                method: "POST",
                path: "/api/devices/:device_id/registers/:name",
                description: "Write single register or coil",
            },
            EndpointInfo {
                method: "GET",
                path: "/ws",
                description: "WebSocket stream for real-time register updates",
            },
        ],
    });

    metrics::record_api_request(
        "GET",
        "/api/info",
        StatusCode::OK.as_u16(),
        start.elapsed().as_secs_f64(),
    );
    resp
}

/// Prometheus metrics endpoint
async fn metrics_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let start = Instant::now();
    match &state.metrics_handle {
        Some(handle) => {
            let metrics_data = handle.render();
            metrics::record_api_request(
                "GET",
                "/metrics",
                StatusCode::OK.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            (
                StatusCode::OK,
                [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
                metrics_data,
            )
        }
        None => {
            metrics::record_api_request(
                "GET",
                "/metrics",
                StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [("content-type", "text/plain; charset=utf-8")],
                "Metrics not enabled".to_string(),
            )
        }
    }
}

// ============================================================================
// Device Endpoints
// ============================================================================

/// Device list response
#[derive(Serialize)]
pub struct DeviceListResponse {
    pub devices: Vec<DeviceSummary>,
    pub count: usize,
}

#[derive(Serialize)]
pub struct DeviceSummary {
    pub id: String,
    pub register_count: usize,
    pub last_update: Option<String>,
}

async fn list_devices(State(state): State<Arc<ApiState>>) -> Json<DeviceListResponse> {
    let start = Instant::now();
    let store = state.register_store.read().await;

    let devices: Vec<DeviceSummary> = store
        .iter()
        .map(|(id, registers)| {
            let last_update = registers
                .values()
                .map(|r| r.timestamp)
                .max()
                .map(|t| t.to_rfc3339());

            DeviceSummary {
                id: id.clone(),
                register_count: registers.len(),
                last_update,
            }
        })
        .collect();

    let count = devices.len();
    metrics::record_api_request(
        "GET",
        "/api/devices",
        StatusCode::OK.as_u16(),
        start.elapsed().as_secs_f64(),
    );
    Json(DeviceListResponse { devices, count })
}

/// Device detail response
#[derive(Serialize)]
pub struct DeviceResponse {
    pub id: String,
    pub registers: Vec<RegisterResponse>,
    pub register_count: usize,
}

#[derive(Serialize, Clone)]
pub struct RegisterResponse {
    pub name: String,
    pub value: f64,
    pub raw: Vec<u16>,
    pub unit: Option<String>,
    pub timestamp: String,
}

async fn get_device(
    State(state): State<Arc<ApiState>>,
    Path(device_id): Path<String>,
) -> Result<Json<DeviceResponse>, (StatusCode, Json<ApiError>)> {
    let start = Instant::now();
    let store = state.register_store.read().await;

    let registers = store.get(&device_id).ok_or_else(|| {
        metrics::record_api_request(
            "GET",
            "/api/devices/:id",
            StatusCode::NOT_FOUND.as_u16(),
            start.elapsed().as_secs_f64(),
        );
        ApiError::new(StatusCode::NOT_FOUND, "Device not found")
    })?;

    let registers: Vec<RegisterResponse> = registers
        .values()
        .map(|r| RegisterResponse {
            name: r.name.clone(),
            value: r.value,
            raw: r.raw.clone(),
            unit: r.unit.clone(),
            timestamp: r.timestamp.to_rfc3339(),
        })
        .collect();

    let register_count = registers.len();
    metrics::record_api_request(
        "GET",
        "/api/devices/:id",
        StatusCode::OK.as_u16(),
        start.elapsed().as_secs_f64(),
    );
    Ok(Json(DeviceResponse {
        id: device_id,
        registers,
        register_count,
    }))
}

/// Detailed live device connection and status response
#[derive(Serialize)]
pub struct DeviceStatusResponse {
    pub device_id: String,
    pub name: String,
    pub device_type: String,
    pub is_connected: bool,
    pub poll_interval_ms: u64,
    pub last_seen: Option<String>,
    pub consecutive_errors: u32,
    pub total_reads: u64,
    pub total_errors: u64,
    pub retry_count: u64,
}

async fn get_device_status(
    State(state): State<Arc<ApiState>>,
    Path(device_id): Path<String>,
) -> Result<Json<DeviceStatusResponse>, (StatusCode, Json<ApiError>)> {
    let start = Instant::now();
    let dev = state.storage.get_device(&device_id).await.map_err(|e| {
        ApiError::with_details(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Storage error",
            e.to_string(),
        )
    })?;

    match dev {
        Some(d) => {
            metrics::record_api_request(
                "GET",
                "/api/devices/:id/status",
                StatusCode::OK.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            Ok(Json(DeviceStatusResponse {
                device_id: d.id,
                name: d.name,
                device_type: d.device_type,
                is_connected: d.is_connected,
                poll_interval_ms: d.poll_interval_ms,
                last_seen: d.last_seen.map(|t| t.to_rfc3339()),
                consecutive_errors: d.consecutive_errors,
                total_reads: d.total_reads,
                total_errors: d.total_errors,
                retry_count: d.retry_count,
            }))
        }
        None => {
            // Check if device is in in-memory store
            let store = state.register_store.read().await;
            if store.contains_key(&device_id) {
                metrics::record_api_request(
                    "GET",
                    "/api/devices/:id/status",
                    StatusCode::OK.as_u16(),
                    start.elapsed().as_secs_f64(),
                );
                Ok(Json(DeviceStatusResponse {
                    device_id: device_id.clone(),
                    name: device_id,
                    device_type: "tcp".to_string(),
                    is_connected: true,
                    poll_interval_ms: 1000,
                    last_seen: Some(Utc::now().to_rfc3339()),
                    consecutive_errors: 0,
                    total_reads: 1,
                    total_errors: 0,
                    retry_count: 0,
                }))
            } else {
                metrics::record_api_request(
                    "GET",
                    "/api/devices/:id/status",
                    StatusCode::NOT_FOUND.as_u16(),
                    start.elapsed().as_secs_f64(),
                );
                Err(ApiError::new(StatusCode::NOT_FOUND, "Device not found"))
            }
        }
    }
}

// ============================================================================
// Telemetry History Endpoint
// ============================================================================

/// Query parameters for telemetry history
#[derive(Deserialize)]
pub struct HistoryQueryParams {
    pub register: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub start: Option<String>,
    pub end: Option<String>,
}

/// Paginated telemetry history response
#[derive(Serialize)]
pub struct HistoryResponse {
    pub device_id: String,
    pub register: Option<String>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub records: Vec<HistoryItem>,
}

#[derive(Serialize)]
pub struct HistoryItem {
    pub device_id: String,
    pub register_name: String,
    pub value: f64,
    pub raw: Vec<u16>,
    pub unit: Option<String>,
    pub timestamp: String,
}

async fn get_device_history(
    State(state): State<Arc<ApiState>>,
    Path(device_id): Path<String>,
    Query(params): Query<HistoryQueryParams>,
) -> Result<Json<HistoryResponse>, (StatusCode, Json<ApiError>)> {
    let start_time = Instant::now();

    // Input validation
    let limit = match params.limit {
        Some(l) if l == 0 || l > 1000 => {
            return Err(ApiError::with_details(
                StatusCode::BAD_REQUEST,
                "Invalid limit parameter",
                "Limit must be between 1 and 1000",
            ));
        }
        Some(l) => l,
        None => 50,
    };

    let offset = params.offset.unwrap_or(0);

    let parsed_start = match params.start {
        Some(s) => match DateTime::parse_from_rfc3339(&s) {
            Ok(dt) => Some(dt.with_timezone(&Utc)),
            Err(e) => {
                return Err(ApiError::with_details(
                    StatusCode::BAD_REQUEST,
                    "Invalid start timestamp format",
                    format!(
                        "Expected RFC3339 timestamp (e.g. 2026-11-20T10:00:00Z): {}",
                        e
                    ),
                ));
            }
        },
        None => None,
    };

    let parsed_end = match params.end {
        Some(s) => match DateTime::parse_from_rfc3339(&s) {
            Ok(dt) => Some(dt.with_timezone(&Utc)),
            Err(e) => {
                return Err(ApiError::with_details(
                    StatusCode::BAD_REQUEST,
                    "Invalid end timestamp format",
                    format!(
                        "Expected RFC3339 timestamp (e.g. 2026-11-20T10:00:00Z): {}",
                        e
                    ),
                ));
            }
        },
        None => None,
    };

    let query = TelemetryQuery {
        device_id: device_id.clone(),
        register_name: params.register.clone(),
        start_time: parsed_start,
        end_time: parsed_end,
        limit,
        offset,
    };

    let result = state.storage.query_telemetry(&query).await.map_err(|e| {
        ApiError::with_details(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Storage query failed",
            e.to_string(),
        )
    })?;

    let records: Vec<HistoryItem> = result
        .records
        .into_iter()
        .map(|r| HistoryItem {
            device_id: r.device_id,
            register_name: r.register_name,
            value: r.value,
            raw: r.raw,
            unit: r.unit,
            timestamp: r.timestamp.to_rfc3339(),
        })
        .collect();

    metrics::record_api_request(
        "GET",
        "/api/devices/:id/history",
        StatusCode::OK.as_u16(),
        start_time.elapsed().as_secs_f64(),
    );

    Ok(Json(HistoryResponse {
        device_id,
        register: params.register,
        total: result.total,
        limit,
        offset,
        records,
    }))
}

// ============================================================================
// Register Endpoints
// ============================================================================

async fn get_registers(
    State(state): State<Arc<ApiState>>,
    Path(device_id): Path<String>,
) -> Result<Json<Vec<RegisterResponse>>, (StatusCode, Json<ApiError>)> {
    let start = Instant::now();
    let store = state.register_store.read().await;

    let registers = store.get(&device_id).ok_or_else(|| {
        metrics::record_api_request(
            "GET",
            "/api/devices/:id/registers",
            StatusCode::NOT_FOUND.as_u16(),
            start.elapsed().as_secs_f64(),
        );
        ApiError::new(StatusCode::NOT_FOUND, "Device not found")
    })?;

    let registers: Vec<RegisterResponse> = registers
        .values()
        .map(|r| RegisterResponse {
            name: r.name.clone(),
            value: r.value,
            raw: r.raw.clone(),
            unit: r.unit.clone(),
            timestamp: r.timestamp.to_rfc3339(),
        })
        .collect();

    metrics::record_api_request(
        "GET",
        "/api/devices/:id/registers",
        StatusCode::OK.as_u16(),
        start.elapsed().as_secs_f64(),
    );
    Ok(Json(registers))
}

async fn get_register(
    State(state): State<Arc<ApiState>>,
    Path((device_id, register_name)): Path<(String, String)>,
) -> Result<Json<RegisterResponse>, (StatusCode, Json<ApiError>)> {
    let start = Instant::now();
    let store = state.register_store.read().await;

    let registers = store.get(&device_id).ok_or_else(|| {
        metrics::record_api_request(
            "GET",
            "/api/devices/:id/registers/:name",
            StatusCode::NOT_FOUND.as_u16(),
            start.elapsed().as_secs_f64(),
        );
        ApiError::new(StatusCode::NOT_FOUND, "Device not found")
    })?;

    let register = registers.get(&register_name).ok_or_else(|| {
        metrics::record_api_request(
            "GET",
            "/api/devices/:id/registers/:name",
            StatusCode::NOT_FOUND.as_u16(),
            start.elapsed().as_secs_f64(),
        );
        ApiError::new(StatusCode::NOT_FOUND, "Register not found")
    })?;

    metrics::record_api_request(
        "GET",
        "/api/devices/:id/registers/:name",
        StatusCode::OK.as_u16(),
        start.elapsed().as_secs_f64(),
    );
    Ok(Json(RegisterResponse {
        name: register.name.clone(),
        value: register.value,
        raw: register.raw.clone(),
        unit: register.unit.clone(),
        timestamp: register.timestamp.to_rfc3339(),
    }))
}

/// Write register request body
#[derive(Deserialize)]
pub struct WriteRegisterRequest {
    pub value: u16,
}

/// Write register response
#[derive(Serialize)]
pub struct WriteRegisterResponse {
    pub success: bool,
    pub device_id: String,
    pub register_name: String,
    pub value_written: u16,
    pub message: String,
}

async fn write_register(
    State(state): State<Arc<ApiState>>,
    Path((device_id, register_name)): Path<(String, String)>,
    Json(payload): Json<WriteRegisterRequest>,
) -> Result<Json<WriteRegisterResponse>, (StatusCode, Json<ApiError>)> {
    let start = Instant::now();

    // Validate device and register exist
    let address = {
        let store = state.register_store.read().await;
        let registers = store.get(&device_id).ok_or_else(|| {
            metrics::record_api_request(
                "POST",
                "/api/devices/:id/registers/:name",
                StatusCode::NOT_FOUND.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            ApiError::new(StatusCode::NOT_FOUND, "Device not found")
        })?;

        let _register = registers.get(&register_name).ok_or_else(|| {
            metrics::record_api_request(
                "POST",
                "/api/devices/:id/registers/:name",
                StatusCode::NOT_FOUND.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            ApiError::new(StatusCode::NOT_FOUND, "Register not found")
        })?;

        0u16
    };

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let write_request = WriteRequest {
        device_id: device_id.clone(),
        address,
        value: payload.value,
        response_tx,
    };

    state.write_tx.send(write_request).await.map_err(|_| {
        metrics::record_api_request(
            "POST",
            "/api/devices/:id/registers/:name",
            StatusCode::SERVICE_UNAVAILABLE.as_u16(),
            start.elapsed().as_secs_f64(),
        );
        ApiError::with_details(
            StatusCode::SERVICE_UNAVAILABLE,
            "Write service unavailable",
            "The Modbus write handler is not running",
        )
    })?;

    let result = tokio::time::timeout(std::time::Duration::from_secs(5), response_rx)
        .await
        .map_err(|_| {
            metrics::record_api_request(
                "POST",
                "/api/devices/:id/registers/:name",
                StatusCode::GATEWAY_TIMEOUT.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            ApiError::with_details(
                StatusCode::GATEWAY_TIMEOUT,
                "Write timeout",
                "The Modbus device did not respond in time",
            )
        })?
        .map_err(|_| {
            metrics::record_api_request(
                "POST",
                "/api/devices/:id/registers/:name",
                StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            ApiError::with_details(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Write failed",
                "Response channel closed unexpectedly",
            )
        })?;

    match result {
        Ok(()) => {
            info!(
                "Write successful: {}:{} = {}",
                device_id, register_name, payload.value
            );
            metrics::record_api_request(
                "POST",
                "/api/devices/:id/registers/:name",
                StatusCode::OK.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            Ok(Json(WriteRegisterResponse {
                success: true,
                device_id,
                register_name,
                value_written: payload.value,
                message: "Register written successfully".to_string(),
            }))
        }
        Err(e) => {
            metrics::record_api_request(
                "POST",
                "/api/devices/:id/registers/:name",
                StatusCode::BAD_GATEWAY.as_u16(),
                start.elapsed().as_secs_f64(),
            );
            Err(ApiError::with_details(
                StatusCode::BAD_GATEWAY,
                "Modbus write failed",
                e,
            ))
        }
    }
}

// ============================================================================
// WebSocket Endpoint
// ============================================================================

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum WsMessage {
    #[serde(rename = "subscribe")]
    Subscribe { devices: Option<Vec<String>> },
    #[serde(rename = "unsubscribe")]
    Unsubscribe,
    #[serde(rename = "update")]
    Update(RegisterUpdate),
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "connected")]
    Connected { message: String },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "pong")]
    Pong,
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<ApiState>>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<ApiState>) {
    let (mut sender, mut receiver) = socket.split();

    let connected_msg = WsMessage::Connected {
        message: format!(
            "RustBridge IoT Platform WebSocket v{}",
            env!("CARGO_PKG_VERSION")
        ),
    };
    if let Ok(msg) = serde_json::to_string(&connected_msg) {
        if sender.send(Message::Text(msg)).await.is_err() {
            return;
        }
    }

    info!("WebSocket client connected");
    let mut update_rx = state.subscribe();
    let mut subscribed_devices: Option<Vec<String>> = None;

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<WsMessage>(&text) {
                            Ok(WsMessage::Subscribe { devices }) => {
                                subscribed_devices = devices;
                                debug!("Client subscribed to devices: {:?}", subscribed_devices);
                            }
                            Ok(WsMessage::Unsubscribe) => {
                                subscribed_devices = Some(vec![]);
                                debug!("Client unsubscribed from updates");
                            }
                            Ok(WsMessage::Ping) => {
                                let pong = serde_json::to_string(&WsMessage::Pong).unwrap();
                                if sender.send(Message::Text(pong)).await.is_err() {
                                    break;
                                }
                            }
                            Ok(_) => {}
                            Err(e) => {
                                warn!("Invalid WebSocket message: {}", e);
                                let error = WsMessage::Error {
                                    message: format!("Invalid message format: {}", e),
                                };
                                if let Ok(msg) = serde_json::to_string(&error) {
                                    let _ = sender.send(Message::Text(msg)).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if sender.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("WebSocket client disconnected");
                        break;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            update = update_rx.recv() => {
                match update {
                    Ok(register_update) => {
                        let should_send = match &subscribed_devices {
                            None => true,
                            Some(devices) if devices.is_empty() => false,
                            Some(devices) => devices.contains(&register_update.device_id),
                        };

                        if should_send {
                            let msg = WsMessage::Update(register_update);
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if sender.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WebSocket client lagged, missed {} updates", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    info!("WebSocket connection closed");
}
