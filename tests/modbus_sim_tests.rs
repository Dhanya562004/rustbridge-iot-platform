//! Modbus TCP Simulator and Device Resilience Tests
//!
//! Validates:
//! - Modbus TCP client communication against an in-process TCP simulator
//! - Resilient non-crashing initialization when device is unreachable
//! - Graceful error handling on network disconnect
//! - Auto-reconnect and recovery when simulated device comes back online

use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use rustbridge::config::{
    ConnectionConfig, DataType, DeviceConfig, DeviceType, RegisterConfig, RegisterType,
    TcpConnection,
};
use rustbridge::modbus::ModbusClient;

/// Helper to build a standard Modbus TCP response for Read Holding Registers (FC 03)
fn build_modbus_tcp_response(request_header: &[u8], values: &[u16]) -> Vec<u8> {
    // Copy Transaction ID (2 bytes) and Protocol ID (2 bytes: Modbus = 0)
    let mut resp = vec![request_header[0], request_header[1], 0x00, 0x00];
    // Length (2 bytes): Unit ID (1) + FC (1) + Byte Count (1) + 2 * registers
    let length = (3 + values.len() * 2) as u16;
    resp.extend_from_slice(&length.to_be_bytes());
    // Unit ID (1 byte)
    resp.push(request_header[6]);
    // Function code (1 byte): Read Holding Registers = 0x03
    resp.push(0x03);
    // Byte count (1 byte)
    resp.push((values.len() * 2) as u8);
    // Register values in Big Endian
    for val in values {
        resp.extend_from_slice(&val.to_be_bytes());
    }
    resp
}

fn create_test_device_config(addr: SocketAddr) -> DeviceConfig {
    let registers = vec![RegisterConfig {
        name: "temperature".to_string(),
        address: 0,
        register_type: RegisterType::Holding,
        count: 1,
        data_type: DataType::I16,
        unit: Some("°C".to_string()),
        scale: Some(0.1),
        offset: Some(0.0),
    }];

    DeviceConfig {
        id: "sim-dev-1".to_string(),
        name: "Simulated Sensor".to_string(),
        device_type: DeviceType::Tcp,
        connection: ConnectionConfig::Tcp(TcpConnection {
            host: addr.ip().to_string(),
            port: addr.port(),
            unit_id: 1,
        }),
        poll_interval_ms: 100,
        registers,
        retry_attempts: 3,
        initial_backoff_ms: 100,
        max_backoff_ms: 1000,
        backoff_multiplier: 1.5,
        timeout_ms: 1500,
    }
}

#[tokio::test]
async fn test_unreachable_device_does_not_crash() {
    // Pick an unused local port where no server is running
    let dummy_addr: SocketAddr = "127.0.0.1:59999".parse().unwrap();
    let config = create_test_device_config(dummy_addr);

    // Initializing client against offline device should NOT panic or crash
    let client = ModbusClient::new(&config).await;
    assert!(
        client.is_ok(),
        "Client should initialize without crashing even if offline"
    );

    let mut client = client.unwrap();
    assert!(!client.is_connected());

    // Reading should gracefully fail with an error
    let result = client.read_registers(&config.registers[0]).await;
    assert!(result.is_err());
    assert!(!client.is_connected());
}

#[tokio::test]
async fn test_modbus_tcp_simulator_successful_read() {
    // Start local TCP mock server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    // Spawn server task that responds to 1 Modbus TCP request
    let server_task = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 256];
            if let Ok(n) = socket.read(&mut buf).await {
                if n >= 12 {
                    // Send response with temperature register value = 250 (25.0 °C)
                    let response = build_modbus_tcp_response(&buf[0..7], &[250]);
                    let _ = socket.write_all(&response).await;
                }
            }
        }
    });

    let config = create_test_device_config(local_addr);
    let mut client = ModbusClient::new(&config).await.unwrap();

    // Perform read
    let result = client.read_registers(&config.registers[0]).await;
    assert!(
        result.is_ok(),
        "Expected successful read from simulator: {:?}",
        result
    );
    let values = result.unwrap();
    assert_eq!(values, vec![250]);
    assert!(client.is_connected());

    let _ = server_task.await;
}

#[tokio::test]
async fn test_simulator_disconnect_and_recovery() {
    // 1. Start server #1
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        // Handle 1 connection then terminate to simulate network drop
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 256];
            if let Ok(n) = socket.read(&mut buf).await {
                if n >= 12 {
                    let response = build_modbus_tcp_response(&buf[0..7], &[100]);
                    let _ = socket.write_all(&response).await;
                }
            }
        }
    });

    let config = create_test_device_config(local_addr);
    let mut client = ModbusClient::new(&config).await.unwrap();

    // Initial read succeeds
    let res1 = client.read_registers(&config.registers[0]).await;
    assert!(res1.is_ok());
    assert_eq!(res1.unwrap(), vec![100]);
    let _ = server_handle.await;

    // Next read fails because server closed connection
    let res2 = client.read_registers(&config.registers[0]).await;
    assert!(res2.is_err());
    assert!(!client.is_connected());

    // 2. Restart server on the same port and verify client reconnects!
    let listener2 = TcpListener::bind(local_addr).await.unwrap();
    let server_handle2 = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener2.accept().await {
            let mut buf = [0u8; 256];
            if let Ok(n) = socket.read(&mut buf).await {
                if n >= 12 {
                    let response = build_modbus_tcp_response(&buf[0..7], &[200]);
                    let _ = socket.write_all(&response).await;
                }
            }
        }
    });

    // Small delay to allow listener to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Read should automatically reconnect and succeed
    let res3 = client.read_registers(&config.registers[0]).await;
    assert!(res3.is_ok(), "Expected recovery: {:?}", res3);
    assert_eq!(res3.unwrap(), vec![200]);
    assert!(client.is_connected());

    let _ = server_handle2.await;
}
