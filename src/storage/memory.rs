//! In-memory telemetry storage with ring buffer retention and filtering

use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;

use super::{DeviceRecord, TelemetryQuery, TelemetryQueryResult, TelemetryRecord};

pub struct InMemoryStorage {
    devices: RwLock<HashMap<String, DeviceRecord>>,
    telemetry: RwLock<HashMap<String, VecDeque<TelemetryRecord>>>,
    max_records_per_device: usize,
}

impl InMemoryStorage {
    pub fn new(max_records_per_device: usize) -> Self {
        Self {
            devices: RwLock::new(HashMap::new()),
            telemetry: RwLock::new(HashMap::new()),
            max_records_per_device: if max_records_per_device == 0 {
                1000
            } else {
                max_records_per_device
            },
        }
    }

    pub async fn save_device(&self, device: &DeviceRecord) -> Result<()> {
        let mut devices = self.devices.write().await;
        devices.insert(device.id.clone(), device.clone());
        Ok(())
    }

    pub async fn get_device(&self, device_id: &str) -> Result<Option<DeviceRecord>> {
        let devices = self.devices.read().await;
        Ok(devices.get(device_id).cloned())
    }

    pub async fn list_devices(&self) -> Result<Vec<DeviceRecord>> {
        let devices = self.devices.read().await;
        Ok(devices.values().cloned().collect())
    }

    pub async fn update_device_status(
        &self,
        device_id: &str,
        connected: bool,
        success: bool,
        retry: bool,
    ) -> Result<()> {
        let mut devices = self.devices.write().await;
        let now = Utc::now();
        if let Some(device) = devices.get_mut(device_id) {
            device.is_connected = connected;
            device.updated_at = now;
            if success {
                device.last_seen = Some(now);
                device.total_reads = device.total_reads.saturating_add(1);
                device.consecutive_errors = 0;
            } else {
                device.total_errors = device.total_errors.saturating_add(1);
                device.consecutive_errors = device.consecutive_errors.saturating_add(1);
            }
            if retry {
                device.retry_count = device.retry_count.saturating_add(1);
            }
        }
        Ok(())
    }

    pub async fn save_telemetry(&self, record: &TelemetryRecord) -> Result<()> {
        let mut telemetry = self.telemetry.write().await;
        let queue = telemetry
            .entry(record.device_id.clone())
            .or_insert_with(VecDeque::new);

        if queue.len() >= self.max_records_per_device {
            queue.pop_front();
        }
        queue.push_back(record.clone());
        Ok(())
    }

    pub async fn query_telemetry(&self, query: &TelemetryQuery) -> Result<TelemetryQueryResult> {
        let telemetry = self.telemetry.read().await;

        let empty = VecDeque::new();
        let queue = telemetry.get(&query.device_id).unwrap_or(&empty);

        // Filter records matching criteria (newest first)
        let filtered: Vec<TelemetryRecord> = queue
            .iter()
            .rev()
            .filter(|r| {
                if let Some(reg) = &query.register_name {
                    if &r.register_name != reg {
                        return false;
                    }
                }
                if let Some(start) = query.start_time {
                    if r.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = query.end_time {
                    if r.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let total = filtered.len();
        let limit = if query.limit == 0 {
            50
        } else {
            query.limit.min(1000)
        };
        let offset = query.offset;

        let records = filtered.into_iter().skip(offset).take(limit).collect();

        Ok(TelemetryQueryResult {
            device_id: query.device_id.clone(),
            register: query.register_name.clone(),
            total,
            limit,
            offset,
            records,
        })
    }

    pub async fn total_telemetry_count(&self) -> Result<usize> {
        let telemetry = self.telemetry.read().await;
        let total = telemetry.values().map(|q| q.len()).sum();
        Ok(total)
    }

    pub async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }

    pub fn storage_type(&self) -> &'static str {
        "memory"
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new(2000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[tokio::test]
    async fn test_device_crud_and_status() {
        let storage = InMemoryStorage::new(100);

        let dev = DeviceRecord::new("dev-1", "Test Device", "tcp", 1000);
        storage.save_device(&dev).await.unwrap();

        let retrieved = storage.get_device("dev-1").await.unwrap();
        assert!(retrieved.is_some());
        let d = retrieved.unwrap();
        assert_eq!(d.name, "Test Device");
        assert!(!d.is_connected);

        // Update status to success
        storage
            .update_device_status("dev-1", true, true, false)
            .await
            .unwrap();
        let d = storage.get_device("dev-1").await.unwrap().unwrap();
        assert!(d.is_connected);
        assert_eq!(d.total_reads, 1);
        assert_eq!(d.consecutive_errors, 0);

        // Update status to error with retry
        storage
            .update_device_status("dev-1", false, false, true)
            .await
            .unwrap();
        let d = storage.get_device("dev-1").await.unwrap().unwrap().clone();
        assert!(!d.is_connected);
        assert_eq!(d.total_errors, 1);
        assert_eq!(d.consecutive_errors, 1);
        assert_eq!(d.retry_count, 1);
    }

    #[tokio::test]
    async fn test_telemetry_query_and_pagination() {
        let storage = InMemoryStorage::new(50);
        let now = Utc::now();

        // Ingest 10 records for temp, 10 for pressure
        for i in 0..10 {
            storage
                .save_telemetry(&TelemetryRecord {
                    id: None,
                    device_id: "dev-1".to_string(),
                    register_name: "temperature".to_string(),
                    value: 20.0 + (i as f64),
                    raw: vec![200 + i],
                    unit: Some("°C".to_string()),
                    timestamp: now + Duration::seconds(i as i64),
                })
                .await
                .unwrap();

            storage
                .save_telemetry(&TelemetryRecord {
                    id: None,
                    device_id: "dev-1".to_string(),
                    register_name: "pressure".to_string(),
                    value: 1.0 + (i as f64) * 0.1,
                    raw: vec![10 + i],
                    unit: Some("bar".to_string()),
                    timestamp: now + Duration::seconds(i as i64),
                })
                .await
                .unwrap();
        }

        // Query all for dev-1
        let q = TelemetryQuery {
            device_id: "dev-1".to_string(),
            register_name: None,
            start_time: None,
            end_time: None,
            limit: 5,
            offset: 0,
        };
        let res = storage.query_telemetry(&q).await.unwrap();
        assert_eq!(res.total, 20);
        assert_eq!(res.records.len(), 5);

        // Query with register filter
        let q_temp = TelemetryQuery {
            device_id: "dev-1".to_string(),
            register_name: Some("temperature".to_string()),
            start_time: None,
            end_time: None,
            limit: 5,
            offset: 0,
        };
        let res_temp = storage.query_telemetry(&q_temp).await.unwrap();
        assert_eq!(res_temp.total, 10);
        assert_eq!(res_temp.records.len(), 5);

        // Query with pagination offset
        let q_offset = TelemetryQuery {
            device_id: "dev-1".to_string(),
            register_name: Some("temperature".to_string()),
            start_time: None,
            end_time: None,
            limit: 5,
            offset: 5,
        };
        let res_offset = storage.query_telemetry(&q_offset).await.unwrap();
        assert_eq!(res_offset.records.len(), 5);
        assert_ne!(res_temp.records[0].value, res_offset.records[0].value);
    }

    #[tokio::test]
    async fn test_capacity_bounding() {
        let storage = InMemoryStorage::new(5);
        for i in 0..10 {
            storage
                .save_telemetry(&TelemetryRecord {
                    id: None,
                    device_id: "dev-bounded".to_string(),
                    register_name: "test".to_string(),
                    value: i as f64,
                    raw: vec![i as u16],
                    unit: None,
                    timestamp: Utc::now(),
                })
                .await
                .unwrap();
        }

        let q = TelemetryQuery {
            device_id: "dev-bounded".to_string(),
            register_name: None,
            start_time: None,
            end_time: None,
            limit: 100,
            offset: 0,
        };
        let res = storage.query_telemetry(&q).await.unwrap();
        assert_eq!(res.total, 5);
    }
}
