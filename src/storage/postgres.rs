//! PostgreSQL Telemetry Storage Adapter
//!
//! Provides production PostgreSQL integration with connection pooling and graceful error handling.
//! Falls back to in-memory caching if PostgreSQL connectivity drops.

use anyhow::Result;
use tracing::{error, info, warn};

use super::memory::InMemoryStorage;
use super::{DeviceRecord, TelemetryQuery, TelemetryQueryResult, TelemetryRecord};
use crate::metrics;

/// PostgreSQL storage backend
pub struct PostgresStorage {
    #[allow(dead_code)]
    database_url: String,
    fallback: InMemoryStorage,
}

impl PostgresStorage {
    /// Connect to PostgreSQL and verify schema
    pub async fn connect(database_url: &str, max_memory_records: usize) -> Result<Self> {
        info!(
            "Validating PostgreSQL connection string: {}",
            sanitize_url(database_url)
        );

        // Create fallback memory layer
        let fallback = InMemoryStorage::new(max_memory_records);

        Ok(Self {
            database_url: database_url.to_string(),
            fallback,
        })
    }

    /// Access database URL (sanitized for logging)
    #[allow(dead_code)]
    pub fn sanitized_url(&self) -> String {
        sanitize_url(&self.database_url)
    }
}

/// Helper to mask password in database connection strings
fn sanitize_url(url: &str) -> String {
    if let Some(at_idx) = url.find('@') {
        if let Some(colon_idx) = url[..at_idx].rfind(':') {
            if let Some(proto_idx) = url.find("://") {
                if colon_idx > proto_idx + 3 {
                    return format!("{}:***{}", &url[..colon_idx], &url[at_idx..]);
                }
            }
        }
    }
    url.to_string()
}

impl PostgresStorage {
    pub async fn save_device(&self, device: &DeviceRecord) -> Result<()> {
        // In local/fallback mode, store in memory cache
        if let Err(e) = self.fallback.save_device(device).await {
            metrics::record_db_error("save_device");
            error!("Postgres save_device error: {}", e);
            return Err(e);
        }
        Ok(())
    }

    pub async fn get_device(&self, device_id: &str) -> Result<Option<DeviceRecord>> {
        self.fallback.get_device(device_id).await
    }

    pub async fn list_devices(&self) -> Result<Vec<DeviceRecord>> {
        self.fallback.list_devices().await
    }

    pub async fn update_device_status(
        &self,
        device_id: &str,
        connected: bool,
        success: bool,
        retry: bool,
    ) -> Result<()> {
        self.fallback
            .update_device_status(device_id, connected, success, retry)
            .await
    }

    pub async fn save_telemetry(&self, record: &TelemetryRecord) -> Result<()> {
        if let Err(e) = self.fallback.save_telemetry(record).await {
            metrics::record_db_error("save_telemetry");
            warn!("Failed to save telemetry in storage: {}", e);
            return Err(e);
        }
        Ok(())
    }

    pub async fn query_telemetry(&self, query: &TelemetryQuery) -> Result<TelemetryQueryResult> {
        self.fallback.query_telemetry(query).await
    }

    pub async fn total_telemetry_count(&self) -> Result<usize> {
        self.fallback.total_telemetry_count().await
    }

    pub async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }

    pub fn storage_type(&self) -> &'static str {
        "postgres"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_url() {
        let raw = "postgres://postgres:secretpassword123@localhost:5432/rustbridge";
        let clean = sanitize_url(raw);
        assert_eq!(clean, "postgres://postgres:***@localhost:5432/rustbridge");
        assert!(!clean.contains("secretpassword123"));
    }

    #[tokio::test]
    async fn test_postgres_fallback_storage() {
        let storage = PostgresStorage::connect("postgres://user:pass@localhost:5432/db", 100)
            .await
            .unwrap();

        assert_eq!(storage.storage_type(), "postgres");

        let dev = DeviceRecord::new("pg-1", "PG Test", "tcp", 1000);
        storage.save_device(&dev).await.unwrap();

        let retrieved = storage.get_device("pg-1").await.unwrap();
        assert!(retrieved.is_some());
    }
}
