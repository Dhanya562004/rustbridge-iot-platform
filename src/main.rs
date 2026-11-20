//! RustBridge IoT Platform - Industrial Protocol Bridge
//!
//! High-performance Modbus TCP/RTU to JSON/MQTT gateway and telemetry platform
//! Built with Rust for Industry 4.0 edge and cloud deployments
//!
//! Maintainer: Dhanya

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod api;
mod bridge;
mod config;
mod metrics;
mod modbus;
mod mqtt;
mod storage;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging with tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    print_banner();

    info!(
        "Starting RustBridge IoT Platform v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load configuration with env overrides and validation
    let config = config::load_config()?;
    info!(
        "Configuration loaded: {} device(s) configured, storage backend: '{}'",
        config.devices.len(),
        config.storage.storage_type
    );

    // Initialize bridge orchestration
    let bridge = bridge::Bridge::new(config).await?;

    // Start the bridge
    bridge.run().await?;

    Ok(())
}

fn print_banner() {
    println!(
        r#"
    ╔═══════════════════════════════════════════════════════════════╗
    ║                                                               ║
    ║   ██████╗ ██╗   ██╗███████╗████████╗██████╗ ██████╗ ██╗██████╗ ║
    ║   ██╔══██╗██║   ██║██╔════╝╚══██╔══╝██╔══██╗██╔══██╗██║██╔═══╝ ║
    ║   ██████╔╝██║   ██║███████╗   ██║   ██████╔╝██████╔╝██║██║  █╗ ║
    ║   ██╔══██╗██║   ██║╚════██║   ██║   ██╔══██╗██╔══██╗██║██║  ██╗║
    ║   ██║  ██║╚██████╔╝███████║   ██║   ██████╔╝██║  ██║██║██████╔╝║
    ║   ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝   ╚═════╝ ╚═╝  ╚═╝╚═╝╚═════╝ ║
    ║                                                               ║
    ║   RustBridge IoT Platform                                     ║
    ║   Modbus TCP/RTU → JSON/MQTT Gateway & Telemetry Platform     ║
    ║   Maintainer: Dhanya                                          ║
    ║   https://github.com/Dhanya562004/rustbridge-iot-platform     ║
    ║                                                               ║
    ╚═══════════════════════════════════════════════════════════════╝
    "#
    );
}
