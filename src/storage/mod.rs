//! Telemetry and Device Metadata Storage Module
//!
//! Provides storage implementations for:
//! - In-memory ring buffer storage with pagination and filtering
//! - PostgreSQL persistent storage integration

pub mod memory;
pub mod postgres;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use memory::InMemoryStorage;
pub use postgres::PostgresStorage;

/// Device metadata record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub is_connected: bool,
    pub poll_interval_ms: u64,
    pub last_seen: Option<DateTime<Utc>>,
    pub consecutive_errors: u32,
    pub total_reads: u64,
    pub total_errors: u64,
    pub retry_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DeviceRecord {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        device_type: impl Into<String>,
        poll_interval_ms: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            device_type: device_type.into(),
            is_connected: false,
            poll_interval_ms,
            last_seen: None,
            consecutive_errors: 0,
            total_reads: 0,
            total_errors: 0,
            retry_count: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Telemetry reading record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub device_id: String,
    pub register_name: String,
    pub value: f64,
    pub raw: Vec<u16>,
    pub unit: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Telemetry history query parameters
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryQuery {
    pub device_id: String,
    pub register_name: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: usize,
    pub offset: usize,
}

/// Paginated query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryQueryResult {
    pub device_id: String,
    pub register: Option<String>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub records: Vec<TelemetryRecord>,
}

/// Storage backend enum supporting In-Memory and PostgreSQL storage
#[derive(Clone)]
pub enum StorageBackend {
    Memory(Arc<InMemoryStorage>),
    Postgres(Arc<PostgresStorage>),
}

impl StorageBackend {
    pub async fn save_device(&self, device: &DeviceRecord) -> Result<()> {
        match self {
            Self::Memory(s) => s.save_device(device).await,
            Self::Postgres(s) => s.save_device(device).await,
        }
    }

    pub async fn get_device(&self, device_id: &str) -> Result<Option<DeviceRecord>> {
        match self {
            Self::Memory(s) => s.get_device(device_id).await,
            Self::Postgres(s) => s.get_device(device_id).await,
        }
    }

    pub async fn list_devices(&self) -> Result<Vec<DeviceRecord>> {
        match self {
            Self::Memory(s) => s.list_devices().await,
            Self::Postgres(s) => s.list_devices().await,
        }
    }

    pub async fn update_device_status(
        &self,
        device_id: &str,
        connected: bool,
        success: bool,
        retry: bool,
    ) -> Result<()> {
        match self {
            Self::Memory(s) => {
                s.update_device_status(device_id, connected, success, retry)
                    .await
            }
            Self::Postgres(s) => {
                s.update_device_status(device_id, connected, success, retry)
                    .await
            }
        }
    }

    pub async fn save_telemetry(&self, record: &TelemetryRecord) -> Result<()> {
        match self {
            Self::Memory(s) => s.save_telemetry(record).await,
            Self::Postgres(s) => s.save_telemetry(record).await,
        }
    }

    pub async fn query_telemetry(&self, query: &TelemetryQuery) -> Result<TelemetryQueryResult> {
        match self {
            Self::Memory(s) => s.query_telemetry(query).await,
            Self::Postgres(s) => s.query_telemetry(query).await,
        }
    }

    pub async fn total_telemetry_count(&self) -> Result<usize> {
        match self {
            Self::Memory(s) => s.total_telemetry_count().await,
            Self::Postgres(s) => s.total_telemetry_count().await,
        }
    }

    pub async fn health_check(&self) -> Result<bool> {
        match self {
            Self::Memory(s) => s.health_check().await,
            Self::Postgres(s) => s.health_check().await,
        }
    }

    pub fn storage_type(&self) -> &'static str {
        match self {
            Self::Memory(s) => s.storage_type(),
            Self::Postgres(s) => s.storage_type(),
        }
    }
}

pub type SharedStorage = StorageBackend;

/// Factory to initialize storage based on configuration
pub async fn init_storage(
    storage_type: &str,
    database_url: Option<&str>,
    max_memory_records: usize,
) -> SharedStorage {
    if storage_type.eq_ignore_ascii_case("postgres") {
        if let Some(url) = database_url {
            match PostgresStorage::connect(url, max_memory_records).await {
                Ok(pg) => {
                    tracing::info!("Initialized PostgreSQL telemetry storage backend");
                    return StorageBackend::Postgres(Arc::new(pg));
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect to PostgreSQL ({}): falling back to in-memory storage",
                        e
                    );
                }
            }
        } else {
            tracing::warn!("Postgres storage requested but no database_url configured; using in-memory storage");
        }
    }

    tracing::info!(
        "Initialized In-Memory telemetry storage (capacity: {} records/device)",
        max_memory_records
    );
    StorageBackend::Memory(Arc::new(InMemoryStorage::new(max_memory_records)))
}
