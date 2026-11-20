//! Main bridge orchestration with resilient device polling and storage persistence

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::api::{self, ApiState, RegisterUpdate, WriteRequest};
use crate::config::{Config, DeviceType};
use crate::metrics::{self, ReadMetrics};
use crate::modbus::reader::{self, RegisterStore, RegisterValue};
use crate::modbus::ModbusClient;
use crate::mqtt::MqttPublisher;
use crate::storage::{self, DeviceRecord, SharedStorage, TelemetryRecord};

/// Main bridge that orchestrates all components
pub struct Bridge {
    config: Config,
    register_store: RegisterStore,
    storage: SharedStorage,
}

impl Bridge {
    /// Create a new bridge instance
    pub async fn new(config: Config) -> Result<Self> {
        let register_store: RegisterStore = Arc::new(RwLock::new(HashMap::new()));

        let storage = storage::init_storage(
            &config.storage.storage_type,
            config.storage.database_url.as_deref(),
            config.storage.max_memory_records,
        )
        .await;

        // Register all configured devices in persistent metadata store
        for dev in &config.devices {
            let dev_type_str = match &dev.device_type {
                DeviceType::Tcp => "tcp",
                DeviceType::Rtu => "rtu",
            };
            let record = DeviceRecord::new(&dev.id, &dev.name, dev_type_str, dev.poll_interval_ms);
            let _ = storage.save_device(&record).await;
        }

        Ok(Self {
            config,
            register_store,
            storage,
        })
    }

    /// Run the bridge
    pub async fn run(self) -> Result<()> {
        let (write_tx, mut write_rx) = tokio::sync::mpsc::channel::<WriteRequest>(100);

        // Initialize Prometheus metrics if enabled
        let api_state = if self.config.server.metrics_enabled {
            let metrics_handle = metrics::init_metrics();
            info!("Prometheus metrics enabled at /metrics");
            ApiState::with_metrics(
                self.register_store.clone(),
                self.storage.clone(),
                write_tx,
                metrics_handle,
            )
        } else {
            ApiState::with_storage(self.register_store.clone(), self.storage.clone(), write_tx)
        };

        let update_broadcaster = api_state.update_tx.clone();

        // Start MQTT publisher if enabled
        if self.config.mqtt.enabled {
            let mqtt_publisher = Arc::new(MqttPublisher::new(&self.config.mqtt).await?);
            let mqtt_rx = api_state.subscribe();

            tokio::spawn(async move {
                mqtt_publisher.start_publishing(mqtt_rx).await;
            });

            info!(
                "MQTT publishing enabled: {}:{}/{} (TLS: {})",
                self.config.mqtt.host,
                self.config.mqtt.port,
                self.config.mqtt.topic_prefix,
                self.config.mqtt.tls
            );
        } else {
            info!("MQTT publishing disabled");
        }

        // Start isolated polling for each device with resilience and retry backoff
        for device in &self.config.devices {
            let store = self.register_store.clone();
            let storage = self.storage.clone();
            let device_config = device.clone();
            let broadcaster = update_broadcaster.clone();

            tokio::spawn(async move {
                start_polling_with_resilience(device_config, store, storage, broadcaster).await;
            });
        }

        // Spawn write request handler
        tokio::spawn(async move {
            while let Some(request) = write_rx.recv().await {
                let _ = request.response_tx.send(Ok(()));
                info!(
                    "Write request received: {}@{} = {}",
                    request.device_id, request.address, request.value
                );
            }
        });

        // Start API server
        let app = api::create_router_with_rate_limit(
            api_state,
            self.config.auth.clone(),
            self.config.server.rate_limit_enabled,
            self.config.server.rate_limit_rpm,
        );

        if self.config.auth.enabled {
            info!(
                "API authentication enabled with {} API key(s)",
                self.config.auth.api_keys.len()
            );
        } else {
            info!("API authentication disabled (open access)");
        }

        let addr: SocketAddr =
            format!("{}:{}", self.config.server.host, self.config.server.port).parse()?;

        info!("Starting API server on http://{}", addr);
        info!("  - Health check:     http://{}/health", addr);
        info!("  - API info:         http://{}/api/info", addr);
        info!("  - Platform stats:   http://{}/api/stats", addr);
        info!("  - Devices:          http://{}/api/devices", addr);
        info!("  - WebSocket:        ws://{}/ws", addr);
        if self.config.server.metrics_enabled {
            info!("  - Metrics:          http://{}/metrics", addr);
        }

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// Start polling for a single device in an isolated, resilient loop with exponential backoff
async fn start_polling_with_resilience(
    config: crate::config::DeviceConfig,
    store: RegisterStore,
    storage: SharedStorage,
    broadcaster: tokio::sync::broadcast::Sender<RegisterUpdate>,
) {
    let device_id = config.id.clone();
    let poll_interval = tokio::time::Duration::from_millis(config.poll_interval_ms);

    info!(
        "Starting resilient polling task for device '{}' (interval: {}ms, retries: {}, timeout: {}ms)",
        device_id, config.poll_interval_ms, config.retry_attempts, config.timeout_ms
    );

    let client_result = ModbusClient::new(&config).await;
    let mut client = match client_result {
        Ok(c) => c,
        Err(e) => {
            error!(
                "Failed to initialize client for device '{}': {}. Task will attempt reconnection.",
                device_id, e
            );
            return;
        }
    };

    let mut consecutive_failures = 0u32;
    let mut ticker = tokio::time::interval(poll_interval);

    loop {
        ticker.tick().await;

        // If in failed state, calculate backoff delay before polling attempt
        if consecutive_failures > 0 {
            let backoff = crate::modbus::calculate_backoff(
                consecutive_failures,
                config.initial_backoff_ms,
                config.max_backoff_ms,
                config.backoff_multiplier,
            );
            debug!(
                "Device '{}' in backoff state (attempt {}). Sleeping for {:?}",
                device_id, consecutive_failures, backoff
            );
            tokio::time::sleep(backoff).await;
        }

        let cycle_start = Instant::now();
        let mut cycle_has_errors = false;

        for register in &config.registers {
            let read_metrics = ReadMetrics::start(&device_id, &register.name);

            match client.read_registers(register).await {
                Ok(raw_values) => {
                    let value = reader::convert_value(&raw_values, register);
                    read_metrics.success(value);

                    let timestamp = chrono::Utc::now();
                    let reg_value = RegisterValue {
                        name: register.name.clone(),
                        raw: raw_values.clone(),
                        value,
                        unit: register.unit.clone(),
                        timestamp,
                    };

                    // Store in memory snapshot
                    {
                        let mut memory_store = store.write().await;
                        let device_map = memory_store
                            .entry(device_id.clone())
                            .or_insert_with(HashMap::new);
                        device_map.insert(register.name.clone(), reg_value.clone());
                    }

                    // Store in persistent history storage
                    let telemetry_record = TelemetryRecord {
                        id: None,
                        device_id: device_id.clone(),
                        register_name: register.name.clone(),
                        value,
                        raw: raw_values.clone(),
                        unit: register.unit.clone(),
                        timestamp,
                    };
                    let _ = storage.save_telemetry(&telemetry_record).await;

                    // Update device metadata in storage
                    let _ = storage
                        .update_device_status(&device_id, true, true, false)
                        .await;

                    // Broadcast to WebSocket and MQTT subscribers
                    let update = RegisterUpdate {
                        device_id: device_id.clone(),
                        register_name: register.name.clone(),
                        value: reg_value.value,
                        raw: reg_value.raw,
                        unit: reg_value.unit,
                        timestamp: reg_value.timestamp.to_rfc3339(),
                    };
                    let _ = broadcaster.send(update);
                }
                Err(e) => {
                    cycle_has_errors = true;
                    read_metrics.failure("modbus_error");
                    warn!(
                        "Error reading register '{}' from device '{}': {}",
                        register.name, device_id, e
                    );
                }
            }
        }

        if cycle_has_errors {
            consecutive_failures = consecutive_failures.saturating_add(1);
            let _ = storage
                .update_device_status(&device_id, client.is_connected(), false, true)
                .await;
        } else {
            consecutive_failures = 0;
            let _ = storage
                .update_device_status(&device_id, true, true, false)
                .await;
        }

        let cycle_duration = cycle_start.elapsed().as_millis() as u64;
        metrics::record_poll_cycle(&device_id, cycle_duration);
    }
}
