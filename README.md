# RustBridge IoT Platform

A Rust-powered IoT telemetry gateway that connects Modbus TCP/RTU devices with REST APIs, WebSocket streams, MQTT messaging, and Prometheus monitoring.

[![CI](https://github.com/Dhanya562004/rustbridge-iot-platform/actions/workflows/ci.yml/badge.svg)](https://github.com/Dhanya562004/rustbridge-iot-platform/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/Docker-Ready-blue.svg)](Dockerfile)
[![Tests](https://img.shields.io/badge/Tests-137%20Passed-brightgreen.svg)](#test-results)

---

## Table of Contents

- [Overview](#overview)
- [Problem Statement](#problem-statement)
- [Key Features](#key-features)
- [Architecture](#architecture)
- [Technology Stack](#technology-stack)
- [Project Directory Structure](#project-directory-structure)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Environment Variables](#environment-variables)
- [API Reference](#api-reference)
- [WebSocket Streaming](#websocket-streaming)
- [MQTT Publishing](#mqtt-publishing)
- [Prometheus Metrics](#prometheus-metrics)
- [Docker Compose Deployment](#docker-compose-deployment)
- [Testing & Quality Assurance](#testing--quality-assurance)
- [Test Results](#test-results)
- [Cloud Deployment](#cloud-deployment)
- [Troubleshooting](#troubleshooting)
- [Security Considerations](#security-considerations)
- [Limitations & Roadmap](#limitations--roadmap)
- [Contributing](#contributing)
- [License & Attribution](#license--attribution)
- [Maintainer](#maintainer)

---

## Overview

**RustBridge IoT Platform** is a production-grade, asynchronous industrial IoT edge gateway built in Rust. It bridges the gap between legacy industrial automation protocols (Modbus TCP and Modbus RTU/RS-485) and modern cloud-native architectures.

RustBridge continuously polls industrial field equipment—such as programmable logic controllers (PLCs), power meters, variable frequency drives (VFDs), and environmental sensors—transforms raw binary register frames into typed, calibrated engineering values, and distributes telemetry in real time across REST APIs, WebSocket streams, and MQTT brokers while exposing Prometheus metrics for cluster observability.

---

## Problem Statement

Industrial operations rely extensively on field devices using Modbus protocol, designed in 1979:
1. **Protocol Incompatibility:** Modbus uses 16-bit binary registers with no native support for JSON, HTTP, WebSockets, or cloud message brokers.
2. **Fragility & Blocking:** Traditional gateways often crash or block when a single physical device experiences connection loss or electrical interference on RS-485 lines.
3. **Lack of Persistence & History:** Most gateways only expose point-in-time instantaneous readings, leaving no telemetry audit trail for edge analytics or intermittent disconnects.
4. **Security Vulnerabilities:** Modbus lacks authentication, encryption, and rate limiting, creating severe industrial attack surfaces when directly exposed to IP networks.

**RustBridge** solves these challenges by combining Rust's memory safety and zero-cost abstractions with isolated asynchronous polling loops, exponential backoff with jitter, sliding-window rate limiting, constant-time API-key authentication, and dual in-memory / PostgreSQL persistence.

---

## Key Features

- **Protocol Support:** Modbus TCP and Modbus RTU (serial RS-485 / RS-232); reads Coils, Discrete Inputs, Input Registers, and Holding Registers.
- **Data Conversion & Scaling:** Decodes `U16`, `I16`, `U32`, `I32`, `F32`, and `Bool` with configurable scale factors (`value * scale + offset`) and engineering units.
- **Device Reliability Engine:** Isolated per-device async tasks; configurable exponential backoff with random jitter; per-operation timeouts preventing hung sockets; automatic reconnection without blocking peer devices.
- **Dual Storage Architecture:**
  - *In-Memory Ring Buffer:* Zero-allocation bounded memory buffer for low-latency queries.
  - *PostgreSQL Persistence Adapter:* SQL migrations, connection pooling, and paginated historical queries.
- **High-Performance REST API (Axum):** Full endpoint suite for instantaneous readings, live device status, historical telemetry with pagination, and actuator register writes.
- **Traffic Shaping & Security:** Sliding-window rate limiting (`429 Too Many Requests`), constant-time API key verification, and runtime environment variable overrides.
- **Real-Time WebSocket Gateway:** Topic-based subscriptions for browser dashboards and edge clients.
- **MQTT 3.1.1 / 5 Integration:** Publishes telemetry and device lifecycle status (`online`/`offline`) with configurable QoS (0, 1, 2), retain flags, and TLS encryption (`ca_file`).
- **Prometheus Observability:** Native `/metrics` endpoint tracking poll cycles, failures, latency histograms, database errors, and device connectivity states.

---

## Architecture

```mermaid
flowchart TD
    subgraph Field["Industrial Field Devices"]
        D1["Modbus TCP Device<br/>(e.g., Power Meter :502)"]
        D2["Modbus RTU Device<br/>(e.g., Temperature Sensor COM1)"]
        D3["Modbus TCP Simulator<br/>(Local Testbed :5020)"]
    end

    subgraph Core["RustBridge IoT Platform Core"]
        direction TB
        subgraph PollingEngine["Resilient Polling Engine"]
            T1["Device Task 1<br/>(Timeout + Exponential Backoff)"]
            T2["Device Task 2<br/>(Timeout + Exponential Backoff)"]
            T3["Device Task 3<br/>(Timeout + Exponential Backoff)"]
        end

        subgraph Transformation["Data Processing Layer"]
            CONV["Register Decoder & Scaler<br/>(I16, U32, F32, Bool)"]
            VAL["Validation & Quality Check"]
        end

        subgraph StorageLayer["Dual Storage Backend"]
            MEM["In-Memory Ring Buffer<br/>(Bounded Capacity)"]
            PG["PostgreSQL Storage<br/>(Relational History & Migrations)"]
        end

        subgraph Dispatch["Distribution & Gateways"]
            BC["Broadcast Channel<br/>(tokio::sync::broadcast)"]
        end
    end

    subgraph Northbound["Northbound Protocols & Clients"]
        REST["Axum REST API<br/>(/api/devices, /history, /stats)"]
        WS["WebSocket Stream<br/>(/ws)"]
        MQTT["MQTT Publisher<br/>(TLS / QoS 0, 1, 2)"]
        PROM["Prometheus Metrics<br/>(/metrics)"]
    end

    D1 <-->|Modbus TCP| T1
    D2 <-->|Serial RTU| T2
    D3 <-->|Modbus TCP| T3

    T1 --> CONV
    T2 --> CONV
    T3 --> CONV
    CONV --> VAL
    VAL --> MEM
    VAL --> PG
    VAL --> BC

    BC --> REST
    BC --> WS
    BC --> MQTT
    Core --> PROM
```

---

## Technology Stack

| Layer | Technology | Purpose |
| :--- | :--- | :--- |
| **Language** | Rust 1.85+ (Edition 2021) | Memory safety, zero-cost abstractions, no garbage collector |
| **Async Runtime** | Tokio 1.43 | Non-blocking multi-threaded asynchronous runtime |
| **HTTP Framework** | Axum 0.7 & Tower | Ergonomic, high-throughput REST and WebSocket server |
| **Industrial Protocol** | tokio-modbus 0.14 | Asynchronous Modbus TCP & RTU communication |
| **Serial Bus** | tokio-serial 5.5 | Non-blocking RS-485 / RS-232 serial communication |
| **MQTT Client** | rumqttc 0.25 | Robust async MQTT 3.1.1 / 5 client with TLS support |
| **Database** | SQLx / PostgreSQL 16 | Relational persistence with automated schema migrations |
| **Observability** | Prometheus Exporter & Tracing | Metrics collection and structured logging |
| **Serialization** | Serde & Serde YAML / JSON | High-performance binary and JSON data serialization |
| **Containerization** | Docker & Docker Compose | Multi-stage minimal production containers (~25 MB) |

---

## Project Directory Structure

```text
rustbridge-iot-platform/
├── .github/
│   └── workflows/
│       └── ci.yml               # Multi-stage CI: format, clippy, tests, release build
├── deploy/                      # Production configuration templates
│   ├── grafana/                 # Grafana provisioning & dashboards
│   ├── mosquitto/               # Eclipse Mosquitto MQTT configuration
│   └── prometheus/              # Prometheus scraping configuration
├── migrations/                  # PostgreSQL SQL schema migrations
│   └── 20261120000001_create_telemetry_tables.sql
├── src/                         # Application source code
│   ├── api/                     # REST API & WebSocket module
│   │   ├── auth.rs              # Constant-time API key verification middleware
│   │   ├── mod.rs               # Axum routes, handlers, and WebSocket engine
│   │   └── rate_limit.rs        # Sliding-window rate-limiting middleware
│   ├── modbus/                  # Industrial Modbus communication module
│   │   ├── client.rs            # Modbus TCP/RTU context abstraction
│   │   ├── mod.rs               # ModbusClient with backoff, retry, and timeout logic
│   │   └── reader.rs            # Raw register parsing, scaling, and conversions
│   ├── storage/                 # Telemetry persistence layer
│   │   ├── memory.rs            # High-performance in-memory ring buffer
│   │   ├── mod.rs               # Unified StorageBackend enum & query interfaces
│   │   └── postgres.rs          # PostgreSQL adapter with graceful fallback
│   ├── bridge.rs                # Main orchestration loop and device supervisor
│   ├── config.rs                # Strongly-typed YAML and env configuration loader
│   ├── lib.rs                   # Library crate entry point
│   ├── main.rs                  # Binary entry point and signal handler
│   ├── metrics/                 # Prometheus metrics registry and recording functions
│   └── mqtt/                    # MQTT client, topic builder, and publishing loop
├── tests/                       # Integration test suites
│   ├── api_history_tests.rs     # Device status, history pagination, and stats tests
│   ├── api_tests.rs             # Core REST API endpoint and auth tests
│   ├── device_retry_tests.rs    # Exponential backoff, jitter, and recovery tests
│   ├── modbus_sim_tests.rs      # Modbus TCP mock server & reconnect tests
│   ├── mqtt_integration_tests.rs# MQTT payload serialization and topic tests
│   └── rate_limit_tests.rs      # Rate limit enforcement and 429 response tests
├── .env.example                 # Reference environment variable configuration
├── .gitignore                   # Git ignore specifications
├── Cargo.lock                   # Deterministic dependency graph
├── Cargo.toml                   # Project manifest and package metadata
├── CHANGELOG.md                 # Release notes and version history
├── config.yaml                  # Default runtime gateway configuration
├── DEPLOYMENT.md                # Cloud (Render, Koyeb) & local deployment guide
├── docker-compose.yml           # Multi-container stack (App, Mosquitto, Prometheus, DB)
├── Dockerfile                   # Multi-stage build for minimal Debian runtime image
└── LICENSE                      # MIT Open Source License & attribution
```

---

## Prerequisites

- **Rust:** `1.85.0` or higher (`cargo`, `rustc`, `rustfmt`, `clippy`)
- **Docker & Docker Compose:** Docker Engine 24.0+ and Compose v2
- **PostgreSQL (Optional):** PostgreSQL 14+ if relational storage is desired
- **OpenSSL / pkg-config:** Required for native build on Linux/macOS

---

## Quick Start

### 1. Clone the Repository
```bash
git clone https://github.com/Dhanya562004/rustbridge-iot-platform.git
cd rustbridge-iot-platform
```

### 2. Configure Environment
```bash
cp .env.example .env
```

### 3. Run Locally in Development Mode
```bash
cargo run
```
The gateway will start on `http://localhost:3000`. If physical devices are offline, RustBridge gracefully logs connection warnings and enters exponential backoff retry cycles without terminating.

### 4. Verify Health Status
```bash
curl -s http://localhost:3000/health | jq .
```

---

## Configuration

RustBridge is configured via `config.yaml`. Settings can be overridden dynamically using environment variables.

### Example `config.yaml`:
```yaml
server:
  host: "0.0.0.0"
  port: 3000
  metrics_enabled: true
  rate_limit_enabled: true
  rate_limit_rpm: 120

auth:
  enabled: false
  api_keys:
    - "demo-api-key-12345"
  exclude_paths:
    - "/health"
    - "/metrics"
    - "/api/info"

storage:
  storage_type: "memory"       # "memory" or "postgres"
  max_memory_records: 1000
  database_url: "postgres://rustbridge:rustbridge_dev_password@localhost:5432/rustbridge"

mqtt:
  enabled: true
  host: "localhost"
  port: 1883
  client_id: "rustbridge-edge-01"
  topic_prefix: "rustbridge"
  qos: 1
  retain: false
  tls: false

devices:
  - id: "power-meter-01"
    name: "Schneider PM5350"
    device_type: "tcp"
    connection:
      host: "192.168.1.100"
      port: 502
      unit_id: 1
    poll_interval_ms: 1000
    retry_attempts: 5
    initial_backoff_ms: 500
    max_backoff_ms: 15000
    backoff_multiplier: 2.0
    timeout_ms: 3000
    registers:
      - name: "voltage_phase_a"
        address: 3028
        register_type: "holding"
        count: 2
        data_type: "f32"
        scale: 1.0
        offset: 0.0
        unit: "V"
      - name: "current_phase_a"
        address: 3000
        register_type: "holding"
        count: 2
        data_type: "f32"
        scale: 1.0
        offset: 0.0
        unit: "A"
```

---

## Environment Variables

Every setting can be overridden using environment variables without modifying `config.yaml`:

| Environment Variable | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `RUST_LOG` | String | `info` | Logging verbosity (`error`, `warn`, `info`, `debug`, `trace`) |
| `RUSTBRIDGE_CONFIG` | Path | `config.yaml` | Path to configuration file |
| `RUSTBRIDGE_SERVER_HOST` | String | `0.0.0.0` | API bind address |
| `RUSTBRIDGE_SERVER_PORT` | u16 | `3000` | API listen port |
| `RUSTBRIDGE_SERVER_METRICS_ENABLED` | bool | `true` | Enables `/metrics` endpoint |
| `RUSTBRIDGE_SERVER_RATE_LIMIT_ENABLED` | bool | `false` | Enables sliding window rate limiting |
| `RUSTBRIDGE_SERVER_RATE_LIMIT_RPM` | u32 | `120` | Max requests per minute per client |
| `RUSTBRIDGE_AUTH_ENABLED` | bool | `false` | Requires `X-API-Key` on protected routes |
| `RUSTBRIDGE_AUTH_API_KEYS` | String | (empty) | Comma-separated authorized API keys |
| `RUSTBRIDGE_STORAGE_TYPE` | String | `memory` | Telemetry backend (`memory` or `postgres`) |
| `RUSTBRIDGE_STORAGE_DATABASE_URL` | String | (none) | PostgreSQL connection string |
| `RUSTBRIDGE_STORAGE_MAX_MEMORY_RECORDS`| usize | `1000` | Retention capacity per device in memory |
| `RUSTBRIDGE_MQTT_ENABLED` | bool | `false` | Enables MQTT telemetry streaming |
| `RUSTBRIDGE_MQTT_HOST` | String | `localhost` | MQTT broker hostname or IP |
| `RUSTBRIDGE_MQTT_PORT` | u16 | `1883` | MQTT broker port |
| `RUSTBRIDGE_MQTT_TLS` | bool | `false` | Enables TLS for MQTT broker |
| `RUSTBRIDGE_MQTT_CA_FILE` | Path | (none) | Path to custom CA certificate for TLS |

---

## API Reference

The REST API runs on port 3000 by default. Protected routes require the `X-API-Key: <key>` header when authentication is enabled.

### Endpoint Catalog

| Method | Path | Auth Required | Description |
| :--- | :--- | :--- | :--- |
| `GET` | `/health` | No | System health, uptime, storage type, and device counts |
| `GET` | `/api/info` | No | Platform metadata, maintainer info, and endpoint list |
| `GET` | `/api/stats` | Yes | Global aggregate statistics (records stored, connectivity) |
| `GET` | `/metrics` | No | Prometheus metrics scraping endpoint |
| `GET` | `/api/devices` | Yes | List all configured devices with register counts |
| `GET` | `/api/devices/:id` | Yes | Get instantaneous cached values for all registers |
| `GET` | `/api/devices/:id/status` | Yes | Get live connection state, error counters, and retry stats |
| `GET` | `/api/devices/:id/history` | Yes | Query historical telemetry with pagination and filtering |
| `GET` | `/api/devices/:id/registers` | Yes | List register definitions and current values |
| `GET` | `/api/devices/:id/registers/:name` | Yes | Read a specific register value |
| `POST` | `/api/devices/:id/registers/:name` | Yes | Write a single 16-bit register or coil |
| `GET` | `/ws` | No | WebSocket stream for real-time register updates |

---

### Example API Requests & Responses

#### 1. System Health (`GET /health`)
```bash
curl -s http://localhost:3000/health | jq .
```
```json
{
  "status": "ok",
  "version": "0.2.0",
  "uptime_seconds": 360,
  "storage_type": "memory",
  "devices_total": 2,
  "devices_connected": 2
}
```

#### 2. Device Live Health & Retry Status (`GET /api/devices/:id/status`)
```bash
curl -s http://localhost:3000/api/devices/power-meter-01/status | jq .
```
```json
{
  "device_id": "power-meter-01",
  "name": "Schneider PM5350",
  "device_type": "tcp",
  "is_connected": true,
  "poll_interval_ms": 1000,
  "last_seen": "2026-11-20T12:00:00Z",
  "consecutive_errors": 0,
  "total_reads": 1420,
  "total_errors": 0,
  "retry_count": 0
}
```

#### 3. Historical Telemetry with Pagination (`GET /api/devices/:id/history`)
```bash
curl -s "http://localhost:3000/api/devices/power-meter-01/history?limit=2&offset=0&register=voltage_phase_a" | jq .
```
```json
{
  "device_id": "power-meter-01",
  "register": "voltage_phase_a",
  "total": 450,
  "limit": 2,
  "offset": 0,
  "records": [
    {
      "device_id": "power-meter-01",
      "register_name": "voltage_phase_a",
      "value": 230.4,
      "raw": [17242, 26214],
      "unit": "V",
      "timestamp": "2026-11-20T12:05:01Z"
    },
    {
      "device_id": "power-meter-01",
      "register_name": "voltage_phase_a",
      "value": 230.2,
      "raw": [17242, 13107],
      "unit": "V",
      "timestamp": "2026-11-20T12:05:00Z"
    }
  ]
}
```

#### 4. Platform Aggregate Stats (`GET /api/stats`)
```bash
curl -s http://localhost:3000/api/stats | jq .
```
```json
{
  "uptime_seconds": 360,
  "total_devices": 2,
  "connected_devices": 2,
  "disconnected_devices": 0,
  "total_telemetry_records": 2840,
  "storage_type": "memory",
  "metrics_enabled": true
}
```

#### 5. Write Register (`POST /api/devices/:id/registers/:name`)
```bash
curl -s -X POST http://localhost:3000/api/devices/power-meter-01/registers/reset_counter \
  -H "Content-Type: application/json" \
  -d '{"value": 1}' | jq .
```
```json
{
  "success": true,
  "device_id": "power-meter-01",
  "register_name": "reset_counter",
  "value_written": 1,
  "message": "Register written successfully"
}
```

---

## WebSocket Streaming

Clients can connect to `/ws` for sub-millisecond real-time telemetry updates.

### Subscription Protocol:
```json
// Subscribe to specific devices:
{"type": "subscribe", "devices": ["power-meter-01"]}

// Subscribe to all devices:
{"type": "subscribe", "devices": null}

// Unsubscribe:
{"type": "unsubscribe"}

// Heartbeat ping/pong:
{"type": "ping"}
```

### Event Payload:
```json
{
  "type": "update",
  "data": {
    "device_id": "power-meter-01",
    "register_name": "voltage_phase_a",
    "value": 230.4,
    "raw": [17242, 26214],
    "unit": "V",
    "timestamp": "2026-11-20T12:05:01Z"
  }
}
```

---

## MQTT Publishing

Telemetry is published to standard IoT topic hierarchies:

| Event Type | Topic Structure | Retain | Payload Example |
| :--- | :--- | :--- | :--- |
| **Telemetry Update** | `{prefix}/{device_id}/{register}` | No | `{"value":230.4,"raw":[17242,26214],"unit":"V","timestamp":"2026-11-20T12:00:00Z"}` |
| **Device Status** | `{prefix}/{device_id}/status` | Yes | `"online"` or `"offline"` |

---

## Prometheus Metrics

Scrape `/metrics` to ingest live telemetry into Prometheus:

```text
# HELP rustbridge_poll_cycles_total Total number of device poll cycles executed
# TYPE rustbridge_poll_cycles_total counter
rustbridge_poll_cycles_total{device="power-meter-01"} 1420

# HELP rustbridge_poll_failures_total Total number of device poll failures
# TYPE rustbridge_poll_failures_total counter
rustbridge_poll_failures_total{device="power-meter-01"} 0

# HELP rustbridge_device_status Device connection status (1 = connected, 0 = disconnected)
# TYPE rustbridge_device_status gauge
rustbridge_device_status{device="power-meter-01"} 1

# HELP rustbridge_device_retries_total Total connection retry attempts per device
# TYPE rustbridge_device_retries_total counter
rustbridge_device_retries_total{device="power-meter-01"} 0

# HELP rustbridge_telemetry_records_ingested_total Total telemetry readings stored
# TYPE rustbridge_telemetry_records_ingested_total counter
rustbridge_telemetry_records_ingested_total 2840

# HELP rustbridge_api_requests_total Total HTTP API requests handled
# TYPE rustbridge_api_requests_total counter
rustbridge_api_requests_total{endpoint="/api/devices",method="GET",status="200"} 48
```

---

## Docker Compose Deployment

A complete multi-container production stack is provided, including RustBridge, Eclipse Mosquitto, Prometheus, Grafana, and an optional Modbus simulator:

```bash
# Start full development and monitoring stack:
docker compose --profile dev --profile monitoring up -d
```

### Stack Endpoints:
- **RustBridge API:** `http://localhost:3000`
- **Prometheus UI:** `http://localhost:9090`
- **Grafana Dashboards:** `http://localhost:3001` (`admin` / `rustbridge`)
- **Mosquitto MQTT:** `tcp://localhost:1883`
- **Modbus Simulator:** `tcp://localhost:5020`

---

## Testing & Quality Assurance

The codebase maintains strict quality standards with comprehensive unit, integration, and simulated hardware test suites.

### Run Verification Commands:
```bash
# 1. Check code formatting:
cargo fmt --all -- --check

# 2. Run Clippy linter with warnings denied:
cargo clippy --all-targets --all-features -- -D warnings

# 3. Execute all test targets:
cargo test --all-targets

# 4. Build optimized release binary:
cargo build --release
```

---

## Test Results

Actual test execution results from the latest verification run:

| Test Target / Suite | Total Executed | Passed | Failed | Ignored | Coverage Areas |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **`src/lib.rs` (Unit Tests)** | 48 | 48 | 0 | 0 | Register conversions (U16, I16, U32, I32, F32, Bool), scaling, offset, MQTT QoS, config validation, in-memory ring buffer, URL sanitization |
| **`src/main.rs` (Unit Tests)** | 48 | 48 | 0 | 0 | Binary integration, metrics recording, device status tracking, poll cycle counters, storage fallbacks |
| **`tests/api_tests.rs`** | 24 | 24 | 0 | 0 | REST API endpoints, auth enforcement, wildcard path exclusions, multi-key verification, error JSON structures, register reads/writes |
| **`tests/api_history_tests.rs`** | 4 | 4 | 0 | 0 | Device status endpoint, historical telemetry pagination (`limit`/`offset`), register filtering, time validation, aggregate platform stats |
| **`tests/device_retry_tests.rs`** | 4 | 4 | 0 | 0 | Exponential backoff progression, max backoff clamps, jitter bounds, consecutive error counters, automatic recovery transitions |
| **`tests/mqtt_integration_tests.rs`**| 4 | 4 | 0 | 0 | Topic hierarchy formatting, telemetry payload JSON serialization, QoS mappings, TLS configuration verification |
| **`tests/modbus_sim_tests.rs`** | 3 | 3 | 0 | 0 | In-process Modbus TCP mock server, register frame parsing, graceful offline non-crashing handling, automatic reconnect on socket recovery |
| **`tests/rate_limit_tests.rs`** | 2 | 2 | 0 | 0 | Sliding-window rate limit allowance, enforcement of `429 Too Many Requests`, `Retry-After: 60` headers |
| **Total** | **137** | **137** | **0** | **0** | **100% Pass Rate** |

---

## Cloud Deployment

Detailed step-by-step instructions for deploying RustBridge to free cloud hosting (Render, Koyeb) or connecting managed PostgreSQL (Supabase, Neon) are documented in [DEPLOYMENT.md](DEPLOYMENT.md).

---

## Troubleshooting

### 1. Connection Timeout on Modbus TCP
- **Symptoms:** Logs display `Device 'xyz' polling failure: timeout`.
- **Remedy:** Ensure the target device IP is reachable from the gateway host. Verify firewall rules allow outbound port `502`. If network jitter is high, increase `timeout_ms` in `config.yaml`.

### 2. Rate Limit Exceeded (`429 Too Many Requests`)
- **Symptoms:** API calls return HTTP status 429.
- **Remedy:** Read the `Retry-After` header and back off requests. To increase thresholds, adjust `RUSTBRIDGE_SERVER_RATE_LIMIT_RPM`.

### 3. PostgreSQL Connection Dropped
- **Symptoms:** Logs display `Postgres storage error; falling back to in-memory storage`.
- **Remedy:** Verify database credentials in `RUSTBRIDGE_STORAGE_DATABASE_URL`. RustBridge automatically falls back to in-memory storage to prevent data loss.

---

## Security Considerations

1. **Zero Secret Hardcoding:** API keys, database credentials, and passwords are never hardcoded. All secrets can be injected via environment variables.
2. **Sensitive URL Sanitization:** Database passwords in connection strings are masked (`postgres://user:***@host:5432/db`) in all logs and error traces.
3. **Constant-Time Verification:** API keys are compared using constant-time algorithms to prevent timing side-channel attacks.
4. **Denial-of-Service Defense:** Built-in sliding-window rate limiting prevents API abuse and denial-of-service conditions.
5. **Non-Root Container:** The production Docker container executes under a dedicated unprivileged user (`rustbridge:rustbridge`).

---

## Limitations & Roadmap

### Current Limitations:
- Modbus RTU requires local serial hardware (e.g. `/dev/ttyUSB0` or `COM1`), which is not accessible in standard serverless or cloud container environments (requires an Ethernet-to-Serial gateway).
- SQLite storage is not bundled natively; persistence is handled via In-Memory Ring Buffer or PostgreSQL.

### Roadmap:
- [ ] BACnet/IP and OPC-UA protocol ingestion adapters.
- [ ] Distributed clustering and leader election using Raft.
- [ ] WebAssembly (Wasm) edge data transformation plugins.
- [ ] TLS client certificate authentication (mTLS) for REST API.

---

## Contributing

Contributions are welcome! Please follow these steps:
1. Fork the repository.
2. Create a feature branch: `git checkout -b feature/my-feature`.
3. Verify all checks pass:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-targets
   ```
4. Commit your changes with a clear commit message.
5. Open a Pull Request.

---

## License & Attribution

This project is licensed under the **MIT License** — see the [LICENSE](LICENSE) file for full legal details.

### Upstream Attribution:
RustBridge was originally developed by **Mustafa Sarac** (2025–2026) under the MIT License. Upstream copyright and permissions are preserved in accordance with the MIT License terms.

---

## Maintainer

**Dhanya**
- **GitHub:** [@Dhanya562004](https://github.com/Dhanya562004)
- **Repository:** [https://github.com/Dhanya562004/rustbridge-iot-platform.git](https://github.com/Dhanya562004/rustbridge-iot-platform.git)
