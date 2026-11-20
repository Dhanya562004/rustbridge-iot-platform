//! Configuration management for RustBridge IoT Platform

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,
    /// MQTT broker configuration
    pub mqtt: MqttConfig,
    /// API authentication configuration
    #[serde(default)]
    pub auth: AuthConfig,
    /// Database / Telemetry storage configuration
    #[serde(default)]
    pub storage: StorageConfig,
    /// List of Modbus devices
    pub devices: Vec<DeviceConfig>,
}

/// Storage backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage type: "memory" or "postgres"
    #[serde(default = "StorageConfig::default_storage_type")]
    pub storage_type: String,
    /// Database connection URL for PostgreSQL (e.g. postgres://user:password@localhost:5432/dbname)
    #[serde(default)]
    pub database_url: Option<String>,
    /// Connection pool size for PostgreSQL
    #[serde(default = "StorageConfig::default_pool_size")]
    pub pool_size: u32,
    /// Maximum history records to keep per device in memory
    #[serde(default = "StorageConfig::default_max_memory_records")]
    pub max_memory_records: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: Self::default_storage_type(),
            database_url: None,
            pool_size: Self::default_pool_size(),
            max_memory_records: Self::default_max_memory_records(),
        }
    }
}

impl StorageConfig {
    fn default_storage_type() -> String {
        "memory".to_string()
    }
    fn default_pool_size() -> u32 {
        5
    }
    fn default_max_memory_records() -> usize {
        2000
    }
}

/// API Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable API key authentication
    #[serde(default)]
    pub enabled: bool,
    /// List of valid API keys
    #[serde(default)]
    pub api_keys: Vec<String>,
    /// Paths excluded from authentication (e.g., /health, /metrics)
    #[serde(default = "AuthConfig::default_exclude_paths")]
    pub exclude_paths: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_keys: vec![],
            exclude_paths: Self::default_exclude_paths(),
        }
    }
}

impl AuthConfig {
    fn default_exclude_paths() -> Vec<String> {
        vec!["/health".to_string(), "/metrics".to_string()]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// HTTP API host
    pub host: String,
    /// HTTP API port
    pub port: u16,
    /// Enable metrics endpoint
    pub metrics_enabled: bool,
    /// Enable API rate limiting
    #[serde(default)]
    pub rate_limit_enabled: bool,
    /// Maximum requests per minute per IP or API key
    #[serde(default = "ServerConfig::default_rate_limit_rpm")]
    pub rate_limit_rpm: u32,
}

impl ServerConfig {
    fn default_rate_limit_rpm() -> u32 {
        120
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    /// Enable MQTT publishing
    #[serde(default)]
    pub enabled: bool,
    /// MQTT broker host
    pub host: String,
    /// MQTT broker port
    pub port: u16,
    /// Client ID
    pub client_id: String,
    /// Topic prefix
    pub topic_prefix: String,
    /// QoS level (0, 1, or 2)
    pub qos: u8,
    /// Retain messages (for status updates)
    #[serde(default)]
    pub retain: bool,
    /// Username (optional)
    pub username: Option<String>,
    /// Password (optional)
    pub password: Option<String>,
    /// Enable TLS for secure broker connections
    #[serde(default)]
    pub tls: bool,
    /// Path to custom CA certificate for TLS verification
    pub ca_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Unique device ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Device type: "tcp" or "rtu"
    pub device_type: DeviceType,
    /// Connection settings
    pub connection: ConnectionConfig,
    /// Polling interval in milliseconds
    pub poll_interval_ms: u64,
    /// Registers to read
    pub registers: Vec<RegisterConfig>,
    /// Maximum retry attempts before marking device unhealthy
    #[serde(default = "DeviceConfig::default_retry_attempts")]
    pub retry_attempts: u32,
    /// Initial retry backoff in milliseconds
    #[serde(default = "DeviceConfig::default_initial_backoff_ms")]
    pub initial_backoff_ms: u64,
    /// Maximum retry backoff in milliseconds
    #[serde(default = "DeviceConfig::default_max_backoff_ms")]
    pub max_backoff_ms: u64,
    /// Backoff multiplier
    #[serde(default = "DeviceConfig::default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Read/Write timeout in milliseconds
    #[serde(default = "DeviceConfig::default_timeout_ms")]
    pub timeout_ms: u64,
}

impl DeviceConfig {
    pub fn default_retry_attempts() -> u32 {
        3
    }
    pub fn default_initial_backoff_ms() -> u64 {
        500
    }
    pub fn default_max_backoff_ms() -> u64 {
        10000
    }
    pub fn default_backoff_multiplier() -> f64 {
        2.0
    }
    pub fn default_timeout_ms() -> u64 {
        3000
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Tcp,
    Rtu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConnectionConfig {
    Tcp(TcpConnection),
    Rtu(RtuConnection),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConnection {
    /// Host address
    pub host: String,
    /// Port (default: 502)
    pub port: u16,
    /// Modbus unit ID
    pub unit_id: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuConnection {
    /// Serial port path (e.g., /dev/ttyUSB0, COM1)
    pub port: String,
    /// Baud rate
    pub baud_rate: u32,
    /// Data bits
    pub data_bits: u8,
    /// Stop bits
    pub stop_bits: u8,
    /// Parity: "none", "even", "odd"
    pub parity: String,
    /// Modbus unit ID
    pub unit_id: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterConfig {
    /// Register name
    pub name: String,
    /// Register address
    pub address: u16,
    /// Register type: "holding", "input", "coil", "discrete"
    pub register_type: RegisterType,
    /// Number of registers to read
    pub count: u16,
    /// Data type for interpretation
    pub data_type: DataType,
    /// Unit of measurement (optional)
    pub unit: Option<String>,
    /// Scaling factor (optional)
    pub scale: Option<f64>,
    /// Offset (optional)
    pub offset: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RegisterType {
    Holding,
    Input,
    Coil,
    Discrete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    U16,
    I16,
    U32,
    I32,
    F32,
    Bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
                metrics_enabled: true,
                rate_limit_enabled: false,
                rate_limit_rpm: 120,
            },
            mqtt: MqttConfig {
                enabled: false,
                host: "localhost".to_string(),
                port: 1883,
                client_id: "rustbridge".to_string(),
                topic_prefix: "rustbridge".to_string(),
                qos: 1,
                retain: false,
                username: None,
                password: None,
                tls: false,
                ca_file: None,
            },
            auth: AuthConfig::default(),
            storage: StorageConfig::default(),
            devices: vec![],
        }
    }
}

impl Config {
    /// Validate configuration integrity
    pub fn validate(&self) -> Result<()> {
        if self.server.port == 0 {
            bail!("Server port must be greater than 0");
        }
        if self.auth.enabled && self.auth.api_keys.is_empty() {
            bail!("API key authentication is enabled but no api_keys are configured");
        }
        if self.mqtt.enabled {
            if self.mqtt.port == 0 {
                bail!("MQTT port must be greater than 0");
            }
            if self.mqtt.host.trim().is_empty() {
                bail!("MQTT host cannot be empty when MQTT is enabled");
            }
        }

        let mut seen_device_ids = std::collections::HashSet::new();
        for dev in &self.devices {
            if dev.id.trim().is_empty() {
                bail!("Device ID cannot be empty");
            }
            if !seen_device_ids.insert(dev.id.clone()) {
                bail!("Duplicate device ID found: {}", dev.id);
            }
            if dev.poll_interval_ms < 10 {
                bail!("Device '{}' poll interval must be at least 10ms", dev.id);
            }

            match &dev.connection {
                ConnectionConfig::Tcp(tcp) => {
                    if tcp.host.trim().is_empty() {
                        bail!("Device '{}' TCP host cannot be empty", dev.id);
                    }
                    if tcp.port == 0 {
                        bail!("Device '{}' TCP port must be greater than 0", dev.id);
                    }
                }
                ConnectionConfig::Rtu(rtu) => {
                    if rtu.port.trim().is_empty() {
                        bail!("Device '{}' RTU serial port cannot be empty", dev.id);
                    }
                    if rtu.baud_rate == 0 {
                        bail!("Device '{}' RTU baud rate must be greater than 0", dev.id);
                    }
                }
            }

            let mut seen_registers = std::collections::HashSet::new();
            for reg in &dev.registers {
                if reg.name.trim().is_empty() {
                    bail!("Register name in device '{}' cannot be empty", dev.id);
                }
                if !seen_registers.insert(reg.name.clone()) {
                    bail!(
                        "Duplicate register name '{}' in device '{}'",
                        reg.name,
                        dev.id
                    );
                }
                if reg.count == 0 {
                    bail!("Register count for '{}' must be greater than 0", reg.name);
                }
            }
        }

        Ok(())
    }
}

/// Apply environment variable overrides to configuration
pub fn apply_env_overrides(config: &mut Config) {
    if let Ok(host) = std::env::var("RUSTBRIDGE_SERVER_HOST") {
        config.server.host = host;
    }
    if let Ok(port_str) = std::env::var("RUSTBRIDGE_SERVER_PORT") {
        if let Ok(port) = port_str.parse() {
            config.server.port = port;
        }
    }
    if let Ok(metrics_str) = std::env::var("METRICS_ENABLED") {
        if let Ok(m) = metrics_str.parse() {
            config.server.metrics_enabled = m;
        }
    }
    if let Ok(rate_str) = std::env::var("RUSTBRIDGE_RATE_LIMIT_ENABLED") {
        if let Ok(r) = rate_str.parse() {
            config.server.rate_limit_enabled = r;
        }
    }
    if let Ok(rpm_str) = std::env::var("RUSTBRIDGE_RATE_LIMIT_RPM") {
        if let Ok(rpm) = rpm_str.parse() {
            config.server.rate_limit_rpm = rpm;
        }
    }
    if let Ok(auth_enabled) = std::env::var("RUSTBRIDGE_AUTH_ENABLED") {
        if let Ok(enabled) = auth_enabled.parse() {
            config.auth.enabled = enabled;
        }
    }
    if let Ok(keys_str) = std::env::var("RUSTBRIDGE_API_KEYS") {
        let keys: Vec<String> = keys_str
            .split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        if !keys.is_empty() {
            config.auth.api_keys = keys;
        }
    }
    if let Ok(storage_type) = std::env::var("RUSTBRIDGE_STORAGE_TYPE") {
        config.storage.storage_type = storage_type;
    }
    if let Ok(db_url) =
        std::env::var("DATABASE_URL").or_else(|_| std::env::var("RUSTBRIDGE_DATABASE_URL"))
    {
        config.storage.database_url = Some(db_url);
    }
    if let Ok(mqtt_enabled) = std::env::var("RUSTBRIDGE_MQTT_ENABLED") {
        if let Ok(e) = mqtt_enabled.parse() {
            config.mqtt.enabled = e;
        }
    }
    if let Ok(mqtt_host) =
        std::env::var("MQTT_HOST").or_else(|_| std::env::var("RUSTBRIDGE_MQTT_HOST"))
    {
        config.mqtt.host = mqtt_host;
    }
    if let Ok(mqtt_port_str) =
        std::env::var("MQTT_PORT").or_else(|_| std::env::var("RUSTBRIDGE_MQTT_PORT"))
    {
        if let Ok(p) = mqtt_port_str.parse() {
            config.mqtt.port = p;
        }
    }
    if let Ok(mqtt_user) =
        std::env::var("MQTT_USERNAME").or_else(|_| std::env::var("RUSTBRIDGE_MQTT_USERNAME"))
    {
        config.mqtt.username = Some(mqtt_user);
    }
    if let Ok(mqtt_pass) =
        std::env::var("MQTT_PASSWORD").or_else(|_| std::env::var("RUSTBRIDGE_MQTT_PASSWORD"))
    {
        config.mqtt.password = Some(mqtt_pass);
    }
}

/// Load configuration from file or use defaults, applying environment overrides
pub fn load_config() -> Result<Config> {
    let config_path =
        std::env::var("RUSTBRIDGE_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());

    let mut config: Config = if Path::new(&config_path).exists() {
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path))?;

        serde_yaml::from_str(&content).with_context(|| "Failed to parse config file")?
    } else {
        tracing::warn!("Config file not found, using defaults");
        Config::default()
    };

    apply_env_overrides(&mut config);
    config
        .validate()
        .with_context(|| "Config validation failed")?;

    Ok(config)
}

/// Load configuration from a YAML string (used in tests)
#[cfg(test)]
pub fn load_config_from_str(yaml: &str) -> Result<Config> {
    let mut config: Config =
        serde_yaml::from_str(yaml).with_context(|| "Failed to parse config")?;
    apply_env_overrides(&mut config);
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3000);
        assert!(config.server.metrics_enabled);
        assert!(!config.mqtt.enabled);
        assert_eq!(config.mqtt.host, "localhost");
        assert_eq!(config.mqtt.port, 1883);
        assert_eq!(config.mqtt.qos, 1);
        assert!(!config.mqtt.retain);
        assert!(config.devices.is_empty());
        assert_eq!(config.storage.storage_type, "memory");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_parse_minimal_config() {
        let yaml = r#"
server:
  host: "127.0.0.1"
  port: 8080
  metrics_enabled: false
mqtt:
  host: "broker.emqx.io"
  port: 1883
  client_id: "test"
  topic_prefix: "test"
  qos: 0
devices: []
"#;
        let config = load_config_from_str(yaml).unwrap();

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert!(!config.server.metrics_enabled);
        assert_eq!(config.mqtt.host, "broker.emqx.io");
        assert_eq!(config.mqtt.qos, 0);
    }

    #[test]
    fn test_parse_tcp_device() {
        let yaml = r#"
server:
  host: "0.0.0.0"
  port: 3000
  metrics_enabled: true
mqtt:
  host: "localhost"
  port: 1883
  client_id: "test"
  topic_prefix: "test"
  qos: 1
devices:
  - id: "plc-01"
    name: "Main PLC"
    device_type: tcp
    connection:
      host: "192.168.1.100"
      port: 502
      unit_id: 1
    poll_interval_ms: 500
    registers:
      - name: "temperature"
        address: 100
        register_type: holding
        count: 1
        data_type: u16
        unit: "°C"
        scale: 0.1
        offset: -10.0
"#;
        let config = load_config_from_str(yaml).unwrap();

        assert_eq!(config.devices.len(), 1);
        let dev = &config.devices[0];
        assert_eq!(dev.id, "plc-01");
        assert_eq!(dev.name, "Main PLC");
        assert!(matches!(dev.device_type, DeviceType::Tcp));
        assert_eq!(dev.poll_interval_ms, 500);
        assert_eq!(dev.retry_attempts, 3);
        assert_eq!(dev.timeout_ms, 3000);

        match &dev.connection {
            ConnectionConfig::Tcp(tcp) => {
                assert_eq!(tcp.host, "192.168.1.100");
                assert_eq!(tcp.port, 502);
                assert_eq!(tcp.unit_id, 1);
            }
            _ => panic!("Expected TCP connection"),
        }

        assert_eq!(dev.registers.len(), 1);
        let reg = &dev.registers[0];
        assert_eq!(reg.name, "temperature");
        assert_eq!(reg.address, 100);
        assert!(matches!(reg.register_type, RegisterType::Holding));
        assert_eq!(reg.scale, Some(0.1));
        assert_eq!(reg.offset, Some(-10.0));
    }

    #[test]
    fn test_parse_rtu_device() {
        let yaml = r#"
server:
  host: "0.0.0.0"
  port: 3000
  metrics_enabled: true
mqtt:
  host: "localhost"
  port: 1883
  client_id: "test"
  topic_prefix: "test"
  qos: 1
devices:
  - id: "serial-sensor"
    name: "RTU Sensor"
    device_type: rtu
    connection:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      stop_bits: 1
      parity: "none"
      unit_id: 2
    poll_interval_ms: 2000
    registers:
      - name: "humidity"
        address: 0
        register_type: input
        count: 1
        data_type: i16
"#;
        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.devices.len(), 1);
        let dev = &config.devices[0];
        assert!(matches!(dev.device_type, DeviceType::Rtu));
    }

    #[test]
    fn test_all_register_types() {
        let yaml = r#"
server:
  host: "0.0.0.0"
  port: 3000
  metrics_enabled: true
mqtt:
  host: "localhost"
  port: 1883
  client_id: "test"
  topic_prefix: "test"
  qos: 1
devices:
  - id: "dev"
    name: "Device"
    device_type: tcp
    connection:
      host: "localhost"
      port: 502
      unit_id: 1
    poll_interval_ms: 1000
    registers:
      - name: "reg_holding"
        address: 0
        register_type: holding
        count: 1
        data_type: u16
      - name: "reg_input"
        address: 1
        register_type: input
        count: 1
        data_type: u16
      - name: "reg_coil"
        address: 2
        register_type: coil
        count: 1
        data_type: bool
      - name: "reg_discrete"
        address: 3
        register_type: discrete
        count: 1
        data_type: bool
"#;
        let config = load_config_from_str(yaml).unwrap();
        let regs = &config.devices[0].registers;
        assert_eq!(regs.len(), 4);
        assert!(matches!(regs[0].register_type, RegisterType::Holding));
        assert!(matches!(regs[1].register_type, RegisterType::Input));
        assert!(matches!(regs[2].register_type, RegisterType::Coil));
        assert!(matches!(regs[3].register_type, RegisterType::Discrete));
    }

    #[test]
    fn test_all_data_types() {
        let yaml = r#"
server:
  host: "0.0.0.0"
  port: 3000
  metrics_enabled: true
mqtt:
  host: "localhost"
  port: 1883
  client_id: "test"
  topic_prefix: "test"
  qos: 1
devices:
  - id: "dev"
    name: "Device"
    device_type: tcp
    connection:
      host: "localhost"
      port: 502
      unit_id: 1
    poll_interval_ms: 1000
    registers:
      - name: "u16_val"
        address: 0
        register_type: holding
        count: 1
        data_type: u16
      - name: "i16_val"
        address: 1
        register_type: holding
        count: 1
        data_type: i16
      - name: "u32_val"
        address: 2
        register_type: holding
        count: 2
        data_type: u32
      - name: "i32_val"
        address: 4
        register_type: holding
        count: 2
        data_type: i32
      - name: "f32_val"
        address: 6
        register_type: holding
        count: 2
        data_type: f32
      - name: "bool_val"
        address: 8
        register_type: holding
        count: 1
        data_type: bool
"#;
        let config = load_config_from_str(yaml).unwrap();
        let regs = &config.devices[0].registers;
        assert_eq!(regs.len(), 6);
        assert!(matches!(regs[0].data_type, DataType::U16));
        assert!(matches!(regs[1].data_type, DataType::I16));
        assert!(matches!(regs[2].data_type, DataType::U32));
        assert!(matches!(regs[3].data_type, DataType::I32));
        assert!(matches!(regs[4].data_type, DataType::F32));
        assert!(matches!(regs[5].data_type, DataType::Bool));
    }

    #[test]
    fn test_invalid_yaml() {
        let yaml = "this is not valid yaml: [";
        let result = load_config_from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_mqtt_with_auth() {
        let yaml = r#"
server:
  host: "0.0.0.0"
  port: 3000
  metrics_enabled: true
mqtt:
  host: "mqtt.secure.com"
  port: 8883
  client_id: "secure-client"
  topic_prefix: "secure"
  qos: 2
  username: "admin"
  password: "secret123"
devices: []
"#;
        let config = load_config_from_str(yaml).unwrap();
        assert_eq!(config.mqtt.username, Some("admin".to_string()));
        assert_eq!(config.mqtt.password, Some("secret123".to_string()));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.server.port, config.server.port);
        assert_eq!(parsed.mqtt.host, config.mqtt.host);
    }

    #[test]
    fn test_validation_errors() {
        let mut invalid = Config::default();
        invalid.server.port = 0;
        assert!(invalid.validate().is_err());

        let mut invalid_auth = Config::default();
        invalid_auth.auth.enabled = true;
        invalid_auth.auth.api_keys = vec![];
        assert!(invalid_auth.validate().is_err());

        let mut invalid_poll = Config::default();
        invalid_poll.devices.push(DeviceConfig {
            id: "d1".to_string(),
            name: "Dev 1".to_string(),
            device_type: DeviceType::Tcp,
            connection: ConnectionConfig::Tcp(TcpConnection {
                host: "localhost".to_string(),
                port: 502,
                unit_id: 1,
            }),
            poll_interval_ms: 2, // < 10
            registers: vec![],
            retry_attempts: 3,
            initial_backoff_ms: 500,
            max_backoff_ms: 10000,
            backoff_multiplier: 2.0,
            timeout_ms: 3000,
        });
        assert!(invalid_poll.validate().is_err());
    }
}
