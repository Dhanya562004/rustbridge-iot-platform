//! Modbus protocol handling
//!
//! Supports both TCP and RTU (serial) connections with automatic reconnection,
//! exponential backoff retries, operation timeouts, and connection status tracking.

use anyhow::{bail, Context as AnyhowContext, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;
use tokio_modbus::prelude::*;
use tokio_serial::SerialPortBuilderExt;
use tracing::{debug, error, info, warn};

use crate::config::{ConnectionConfig, DeviceConfig, RegisterConfig, RegisterType};
use crate::metrics;

pub mod client;
pub mod reader;

/// Device runtime health and connection status
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct DeviceRuntimeStatus {
    pub device_id: String,
    pub device_type: String,
    pub is_connected: bool,
    pub last_successful_read: Option<DateTime<Utc>>,
    pub consecutive_errors: u32,
    pub total_reads: u64,
    pub total_errors: u64,
    pub retry_count: u64,
    pub poll_interval_ms: u64,
}

/// Modbus client abstraction supporting TCP and RTU with automatic reconnection
pub struct ModbusClient {
    device_id: String,
    device_type: String,
    config: DeviceConfig,
    context: Option<client::Context>,
    is_connected: bool,
    consecutive_errors: u32,
    total_reads: u64,
    total_errors: u64,
    retry_count: u64,
    last_success: Option<DateTime<Utc>>,
}

impl ModbusClient {
    /// Create a new Modbus client from device configuration.
    /// Attempts initial connection, but does not crash if device is offline.
    pub async fn new(config: &DeviceConfig) -> Result<Self> {
        let device_type_str = match &config.connection {
            ConnectionConfig::Tcp(_) => "TCP".to_string(),
            ConnectionConfig::Rtu(_) => "RTU".to_string(),
        };

        let mut client = Self {
            device_id: config.id.clone(),
            device_type: device_type_str,
            config: config.clone(),
            context: None,
            is_connected: false,
            consecutive_errors: 0,
            total_reads: 0,
            total_errors: 0,
            retry_count: 0,
            last_success: None,
        };

        // Attempt initial connection
        if let Err(e) = client.connect().await {
            warn!(
                "Initial connection to Modbus device '{}' failed (will retry): {}",
                client.device_id, e
            );
        }

        Ok(client)
    }

    /// Establish connection to Modbus device with timeout
    pub async fn connect(&mut self) -> Result<()> {
        info!(
            "Connecting to Modbus device '{}' ({})",
            self.device_id, self.device_type
        );
        let timeout_dur = Duration::from_millis(self.config.timeout_ms);

        let result = timeout(timeout_dur, async {
            match &self.config.connection {
                ConnectionConfig::Tcp(tcp) => {
                    let addr: SocketAddr = format!("{}:{}", tcp.host, tcp.port)
                        .parse()
                        .with_context(|| {
                            format!("Invalid TCP address {}:{}", tcp.host, tcp.port)
                        })?;

                    debug!("Opening TCP socket to {} (slave {})", addr, tcp.unit_id);
                    let ctx = tcp::connect_slave(addr, Slave(tcp.unit_id))
                        .await
                        .with_context(|| format!("Failed to connect to {}", addr))?;

                    Ok(client::Context::Tcp(ctx))
                }
                ConnectionConfig::Rtu(rtu) => {
                    let parity = match rtu.parity.to_lowercase().as_str() {
                        "even" => tokio_serial::Parity::Even,
                        "odd" => tokio_serial::Parity::Odd,
                        _ => tokio_serial::Parity::None,
                    };

                    let stop_bits = match rtu.stop_bits {
                        2 => tokio_serial::StopBits::Two,
                        _ => tokio_serial::StopBits::One,
                    };

                    let data_bits = match rtu.data_bits {
                        5 => tokio_serial::DataBits::Five,
                        6 => tokio_serial::DataBits::Six,
                        7 => tokio_serial::DataBits::Seven,
                        _ => tokio_serial::DataBits::Eight,
                    };

                    let builder = tokio_serial::new(&rtu.port, rtu.baud_rate)
                        .parity(parity)
                        .stop_bits(stop_bits)
                        .data_bits(data_bits);

                    let port = builder.open_native_async().with_context(|| {
                        format!(
                            "Failed to open serial port {} @ {} baud",
                            rtu.port, rtu.baud_rate
                        )
                    })?;

                    let ctx = rtu::attach_slave(port, Slave(rtu.unit_id));
                    Ok(client::Context::Rtu(ctx))
                }
            }
        })
        .await;

        match result {
            Ok(Ok(ctx)) => {
                self.context = Some(ctx);
                self.is_connected = true;
                metrics::record_device_status(&self.device_id, true);
                info!("Device '{}' connected successfully", self.device_id);
                Ok(())
            }
            Ok(Err(e)) => {
                self.disconnect();
                metrics::record_device_status(&self.device_id, false);
                Err(e)
            }
            Err(_) => {
                self.disconnect();
                metrics::record_device_status(&self.device_id, false);
                bail!("Connection timeout after {}ms", self.config.timeout_ms)
            }
        }
    }

    /// Disconnect context and reset state
    pub fn disconnect(&mut self) {
        self.context = None;
        self.is_connected = false;
        metrics::record_device_status(&self.device_id, false);
    }

    /// Ensure active connection before operations
    pub async fn ensure_connection(&mut self) -> Result<()> {
        if !self.is_connected || self.context.is_none() {
            self.retry_count = self.retry_count.saturating_add(1);
            metrics::record_device_retry(&self.device_id);
            self.connect().await?;
        }
        Ok(())
    }

    /// Read registers with timeout, automatic reconnection, and status tracking
    pub async fn read_registers(&mut self, register: &RegisterConfig) -> Result<Vec<u16>> {
        if let Err(e) = self.ensure_connection().await {
            self.handle_failure("connection_failed");
            return Err(e);
        }

        let timeout_dur = Duration::from_millis(self.config.timeout_ms);
        let ctx = self
            .context
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No active Modbus connection"))?;

        let read_future = async {
            match register.register_type {
                RegisterType::Holding => ctx
                    .read_holding_registers(register.address, register.count)
                    .await
                    .map_err(|e| anyhow::anyhow!("Modbus error: {}", e)),
                RegisterType::Input => ctx
                    .read_input_registers(register.address, register.count)
                    .await
                    .map_err(|e| anyhow::anyhow!("Modbus error: {}", e)),
                RegisterType::Coil => {
                    let coils = ctx
                        .read_coils(register.address, register.count)
                        .await
                        .map_err(|e| anyhow::anyhow!("Modbus error: {}", e))?;
                    Ok(coils.iter().map(|&b| if b { 1u16 } else { 0u16 }).collect())
                }
                RegisterType::Discrete => {
                    let inputs = ctx
                        .read_discrete_inputs(register.address, register.count)
                        .await
                        .map_err(|e| anyhow::anyhow!("Modbus error: {}", e))?;
                    Ok(inputs
                        .iter()
                        .map(|&b| if b { 1u16 } else { 0u16 })
                        .collect())
                }
            }
        };

        match timeout(timeout_dur, read_future).await {
            Ok(Ok(values)) => {
                self.handle_success();
                metrics::record_telemetry_ingested(&self.device_id, &register.name);
                Ok(values)
            }
            Ok(Err(e)) => {
                self.handle_failure("modbus_error");
                self.disconnect(); // force reconnect on transport error
                Err(e)
            }
            Err(_) => {
                self.handle_failure("timeout");
                self.disconnect();
                bail!("Read timeout after {}ms", self.config.timeout_ms)
            }
        }
    }

    /// Write a single register
    #[allow(dead_code)]
    pub async fn write_register(&mut self, address: u16, value: u16) -> Result<()> {
        self.ensure_connection().await?;
        let timeout_dur = Duration::from_millis(self.config.timeout_ms);

        let ctx = self
            .context
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No active Modbus connection"))?;

        let write_fut = ctx.write_single_register(address, value);

        match timeout(timeout_dur, write_fut).await {
            Ok(Ok(())) => {
                info!(
                    "Wrote register {} = {} on '{}'",
                    address, value, self.device_id
                );
                Ok(())
            }
            Ok(Err(e)) => {
                self.disconnect();
                Err(anyhow::anyhow!("Modbus write error: {}", e))
            }
            Err(_) => {
                self.disconnect();
                bail!("Write timeout after {}ms", self.config.timeout_ms)
            }
        }
    }

    /// Write multiple registers
    #[allow(dead_code)]
    pub async fn write_registers(&mut self, address: u16, values: &[u16]) -> Result<()> {
        self.ensure_connection().await?;
        let timeout_dur = Duration::from_millis(self.config.timeout_ms);

        let ctx = self
            .context
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No active Modbus connection"))?;

        let write_fut = ctx.write_multiple_registers(address, values);

        match timeout(timeout_dur, write_fut).await {
            Ok(Ok(())) => {
                info!(
                    "Wrote {} registers starting at {} on '{}'",
                    values.len(),
                    address,
                    self.device_id
                );
                Ok(())
            }
            Ok(Err(e)) => {
                self.disconnect();
                Err(anyhow::anyhow!("Modbus write error: {}", e))
            }
            Err(_) => {
                self.disconnect();
                bail!("Write timeout after {}ms", self.config.timeout_ms)
            }
        }
    }

    /// Write a single coil
    #[allow(dead_code)]
    pub async fn write_coil(&mut self, address: u16, value: bool) -> Result<()> {
        self.ensure_connection().await?;
        let timeout_dur = Duration::from_millis(self.config.timeout_ms);

        let ctx = self
            .context
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No active Modbus connection"))?;

        let write_fut = ctx.write_single_coil(address, value);

        match timeout(timeout_dur, write_fut).await {
            Ok(Ok(())) => {
                info!("Wrote coil {} = {} on '{}'", address, value, self.device_id);
                Ok(())
            }
            Ok(Err(e)) => {
                self.disconnect();
                Err(anyhow::anyhow!("Modbus write coil error: {}", e))
            }
            Err(_) => {
                self.disconnect();
                bail!("Write coil timeout after {}ms", self.config.timeout_ms)
            }
        }
    }

    fn handle_success(&mut self) {
        self.is_connected = true;
        self.consecutive_errors = 0;
        self.total_reads = self.total_reads.saturating_add(1);
        self.last_success = Some(Utc::now());
        metrics::record_device_status(&self.device_id, true);
    }

    fn handle_failure(&mut self, error_type: &str) {
        self.consecutive_errors = self.consecutive_errors.saturating_add(1);
        self.total_errors = self.total_errors.saturating_add(1);
        metrics::record_poll_failure(&self.device_id);
        if self.consecutive_errors >= self.config.retry_attempts {
            self.is_connected = false;
            metrics::record_device_status(&self.device_id, false);
        }
        error!(
            "Device '{}' polling failure ({}, consecutive: {}): error_type={}",
            self.device_id, self.total_errors, self.consecutive_errors, error_type
        );
    }

    /// Check if connection is active
    pub fn is_connected(&self) -> bool {
        self.is_connected && self.context.is_some()
    }

    /// Device type
    #[allow(dead_code)]
    pub fn device_type(&self) -> &str {
        &self.device_type
    }

    /// Return live runtime status
    #[allow(dead_code)]
    pub fn runtime_status(&self) -> DeviceRuntimeStatus {
        DeviceRuntimeStatus {
            device_id: self.device_id.clone(),
            device_type: self.device_type.clone(),
            is_connected: self.is_connected,
            last_successful_read: self.last_success,
            consecutive_errors: self.consecutive_errors,
            total_reads: self.total_reads,
            total_errors: self.total_errors,
            retry_count: self.retry_count,
            poll_interval_ms: self.config.poll_interval_ms,
        }
    }
}

/// Calculate exponential backoff duration based on consecutive attempts
pub fn calculate_backoff(attempt: u32, initial_ms: u64, max_ms: u64, multiplier: f64) -> Duration {
    if attempt == 0 {
        return Duration::from_millis(initial_ms);
    }
    let factor = multiplier.powi((attempt - 1).min(10) as i32);
    let ms = (initial_ms as f64 * factor) as u64;
    Duration::from_millis(ms.min(max_ms))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DataType, RtuConnection, TcpConnection};

    #[test]
    fn test_backoff_calculation() {
        let b0 = calculate_backoff(0, 500, 10000, 2.0);
        assert_eq!(b0, Duration::from_millis(500));

        let b1 = calculate_backoff(1, 500, 10000, 2.0);
        assert_eq!(b1, Duration::from_millis(500));

        let b2 = calculate_backoff(2, 500, 10000, 2.0);
        assert_eq!(b2, Duration::from_millis(1000));

        let b3 = calculate_backoff(3, 500, 10000, 2.0);
        assert_eq!(b3, Duration::from_millis(2000));

        // Test clamping to max_ms
        let b_huge = calculate_backoff(10, 500, 5000, 2.0);
        assert_eq!(b_huge, Duration::from_millis(5000));
    }

    #[test]
    fn test_tcp_connection_config() {
        let tcp = TcpConnection {
            host: "192.168.1.10".to_string(),
            port: 502,
            unit_id: 1,
        };
        assert_eq!(tcp.host, "192.168.1.10");
        assert_eq!(tcp.port, 502);
        assert_eq!(tcp.unit_id, 1);
    }

    #[test]
    fn test_rtu_connection_config() {
        let rtu = RtuConnection {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 115200,
            data_bits: 8,
            stop_bits: 1,
            parity: "even".to_string(),
            unit_id: 5,
        };
        assert_eq!(rtu.port, "/dev/ttyUSB0");
        assert_eq!(rtu.baud_rate, 115200);
        assert_eq!(rtu.parity, "even");
    }

    #[test]
    fn test_register_config() {
        let reg = RegisterConfig {
            name: "speed".to_string(),
            address: 10,
            register_type: RegisterType::Holding,
            count: 2,
            data_type: DataType::U32,
            unit: Some("RPM".to_string()),
            scale: Some(0.5),
            offset: None,
        };
        assert_eq!(reg.name, "speed");
        assert_eq!(reg.count, 2);
    }
}
