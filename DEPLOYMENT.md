# RustBridge IoT Platform - Cloud & Local Deployment Guide

This guide covers step-by-step production and staging deployment of the **RustBridge IoT Platform**, including **free-tier cloud deployment** (Render & Koyeb) and **local multi-container deployment** via Docker Compose.

---

## Table of Contents

1. [Deployment Architecture](#deployment-architecture)
2. [Free Cloud Hosting Provider Analysis](#free-cloud-hosting-provider-analysis)
3. [Cloud Deployment on Render (Step-by-Step)](#cloud-deployment-on-render-step-by-step)
4. [Alternative Cloud Deployment: Koyeb](#alternative-cloud-deployment-koyeb)
5. [Cloud Database Setup (PostgreSQL)](#cloud-database-setup-postgresql)
6. [Cloud MQTT Broker Setup](#cloud-mqtt-broker-setup)
7. [Health Check & Verification](#health-check--verification)
8. [API & WebSocket Verification](#api--websocket-verification)
9. [Prometheus Metrics Verification](#prometheus-metrics-verification)
10. [Local Production Deployment (Docker Compose)](#local-production-deployment-docker-compose)
11. [Free-Tier Restrictions & Real-World Caveats](#free-tier-restrictions--real-world-caveats)
12. [Troubleshooting Guide](#troubleshooting-guide)

---

## 1. Deployment Architecture

In cloud environments where physical RS-485 serial buses or local industrial Ethernet switches are inaccessible, RustBridge connects to Modbus TCP devices via simulated instances, VPN gateways, or edge device tunnels:

```mermaid
flowchart TD
    subgraph Edge["Field / Edge / Simulation Layer"]
        M1["Modbus TCP Simulator / Gateway<br/>(Port 502 / 5020)"]
        MQTT_B["HiveMQ Cloud / EMQX<br/>(TLS Port 8883)"]
    end

    subgraph Cloud["Cloud Platform (Render / Koyeb / Docker)"]
        RB["RustBridge IoT Platform<br/>(Rust Binary / Multi-Stage Container)"]
        subgraph Storage["Storage Layer"]
            MEM["In-Memory Buffer<br/>(Ring Buffer)"]
            PG["Managed PostgreSQL<br/>(Telemetry & Device Records)"]
        end
        RB --> MEM
        RB --> PG
    end

    subgraph Clients["Consumers & Dashboards"]
        CURL["REST API Clients<br/>(HTTP / HTTPS)"]
        WS["Live Monitoring UI<br/>(WebSocket /ws)"]
        PROM["Prometheus Scraper<br/>(/metrics)"]
    end

    M1 -.->|Modbus TCP Poll| RB
    RB -.->|MQTT Publish| MQTT_B
    CURL <--> RB
    WS <--> RB
    PROM <--> RB
```

---

## 2. Free Cloud Hosting Provider Analysis

| Provider | Free Tier Resources | Inbound Protocols | Background Polling | Ephemeral Storage | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Render** | 0.1 CPU, 512 MB RAM, Free HTTPS | HTTP/1.1, HTTP/2, WebSocket | Yes (Async Tokio tasks) | Ephemeral (resets on restart) | **Recommended (Easiest setup)** |
| **Koyeb** | 1 Nano Service, 512 MB RAM | HTTP/1.1, WebSocket, gRPC | Yes (continuous) | Ephemeral | **Strong Alternative (No auto-sleep)** |
| **Fly.io** | Limited shared CPU | TCP, UDP, HTTP, WS | Yes | Persistent volumes available | Requires CC verification |
| **Railway** | \$5 trial credit (not permanent free) | HTTP, TCP | Yes | Volumes available | Not permanently free |

### Why Render was Selected as Primary:
1. **Zero-Configuration Docker:** Automatically builds and deploys multi-stage `Dockerfile` directly from GitHub.
2. **Native WebSocket Support:** Full duplex bidirectional streaming over port 443 with TLS termination out of the box.
3. **Outbound Network Freedom:** Allows persistent outbound TCP connections for polling simulated Modbus TCP devices and publishing to external MQTT brokers (HiveMQ, EMQX).
4. **Environment-Driven Configuration:** Full support for `RUSTBRIDGE_*` environment variable overrides.

---

## 3. Cloud Deployment on Render (Step-by-Step)

### Step 1: Fork or Push Repository to GitHub
Ensure your repository is pushed to your GitHub account:
```bash
git remote -v
# origin https://github.com/Dhanya562004/rustbridge-iot-platform.git
```

### Step 2: Sign Up on Render
1. Navigate to [https://render.com](https://render.com).
2. Sign in using your GitHub account (`Dhanya562004`).

### Step 3: Create a New Web Service
1. In the Render Dashboard, click **New +** > **Web Service**.
2. Select **Build and deploy from a Git repository**.
3. Choose `Dhanya562004/rustbridge-iot-platform`.
4. Configure service settings:
   - **Name:** `rustbridge-iot-platform`
   - **Region:** Frankfurt (EU Central) or Oregon (US West)
   - **Branch:** `main`
   - **Runtime:** `Docker` (Render automatically detects the root `Dockerfile`)
   - **Instance Type:** `Free`

### Step 4: Configure Environment Variables
In the **Environment Variables** section on Render, add:

| Key | Value | Description |
| :--- | :--- | :--- |
| `RUST_LOG` | `info,rustbridge=debug` | Tracing log verbosity |
| `RUSTBRIDGE_SERVER_PORT` | `10000` | Render expects services on port 10000 by default |
| `RUSTBRIDGE_SERVER_METRICS_ENABLED` | `true` | Enables `/metrics` endpoint |
| `RUSTBRIDGE_SERVER_RATE_LIMIT_ENABLED`| `true` | Enforces rate limiting |
| `RUSTBRIDGE_SERVER_RATE_LIMIT_RPM` | `120` | 120 requests/minute per client |
| `RUSTBRIDGE_AUTH_ENABLED` | `true` | Enables API key authentication |
| `RUSTBRIDGE_AUTH_API_KEYS` | `secret-prod-token-2026,monitoring-key-99` | Comma-separated API keys |
| `RUSTBRIDGE_STORAGE_TYPE` | `memory` | Use `memory` or `postgres` |

> [!NOTE]
> When using `memory` storage, telemetry is buffered in memory. When connecting a PostgreSQL database, change `RUSTBRIDGE_STORAGE_TYPE` to `postgres` and set `RUSTBRIDGE_STORAGE_DATABASE_URL`.

### Step 5: Deploy
Click **Create Web Service**. Render will clone the repository, run the multi-stage Docker build, and start the service. Your service URL will be:
`https://rustbridge-iot-platform.onrender.com`

---

## 4. Alternative Cloud Deployment: Koyeb

Koyeb offers a free nano instance that does not sleep after inactivity.

1. Create a free account at [https://www.koyeb.com](https://www.koyeb.com).
2. Click **Create App** > **GitHub**.
3. Select `Dhanya562004/rustbridge-iot-platform`.
4. Builder: Select **Dockerfile**.
5. Port: `3000` (Protocol: HTTP).
6. Add environment variables:
   - `RUST_LOG`: `info`
   - `RUSTBRIDGE_SERVER_PORT`: `3000`
   - `RUSTBRIDGE_AUTH_ENABLED`: `false` (for open demo) or `true`
7. Click **Deploy**.

---

## 5. Cloud Database Setup (PostgreSQL)

To enable persistent telemetry storage across cloud restarts:

### Option A: Free Supabase / Neon / Render PostgreSQL
1. Create a free PostgreSQL database on [Supabase](https://supabase.com) or [Neon.tech](https://neon.tech).
2. Copy the connection string:
   ```text
   postgres://postgres:[YOUR-PASSWORD]@db.xxxx.supabase.co:5432/postgres?sslmode=require
   ```
3. Run the migration script located at `migrations/20261120000001_create_telemetry_tables.sql` in the Supabase SQL editor or via `psql`:
   ```bash
   psql "postgres://postgres:password@host:5432/postgres" -f migrations/20261120000001_create_telemetry_tables.sql
   ```
4. Set the environment variable in your cloud web service:
   - `RUSTBRIDGE_STORAGE_TYPE`: `postgres`
   - `RUSTBRIDGE_STORAGE_DATABASE_URL`: `postgres://postgres:password@host:5432/postgres`

---

## 6. Cloud MQTT Broker Setup

For publishing telemetry to an external cloud MQTT broker:

1. Sign up for a free cloud broker such as [HiveMQ Cloud](https://www.hivemq.com/cloud/) or [EMQX Serverless](https://www.emqx.com/en/cloud).
2. Configure credentials and TLS:
   - `RUSTBRIDGE_MQTT_ENABLED`: `true`
   - `RUSTBRIDGE_MQTT_HOST`: `your-instance.hivemq.cloud`
   - `RUSTBRIDGE_MQTT_PORT`: `8883`
   - `RUSTBRIDGE_MQTT_TLS`: `true`
   - `RUSTBRIDGE_MQTT_USERNAME`: `iot_gateway`
   - `RUSTBRIDGE_MQTT_PASSWORD`: `your_secure_password`
   - `RUSTBRIDGE_MQTT_TOPIC_PREFIX`: `factory/telemetry`

---

## 7. Health Check & Verification

Verify that your deployment is live:

```bash
curl -i https://rustbridge-iot-platform.onrender.com/health
```

**Expected Response (`200 OK`):**
```json
{
  "status": "ok",
  "version": "0.2.0",
  "uptime_seconds": 42,
  "storage_type": "memory",
  "devices_total": 2,
  "devices_connected": 0
}
```

---

## 8. API & WebSocket Verification

### 8.1 API Catalog Metadata
```bash
curl -s https://rustbridge-iot-platform.onrender.com/api/info | jq .
```

### 8.2 Device Health and Connection Status
```bash
curl -s -H "X-API-Key: secret-prod-token-2026" \
  https://rustbridge-iot-platform.onrender.com/api/devices/power-meter-01/status | jq .
```

**Response:**
```json
{
  "device_id": "power-meter-01",
  "name": "Schneider Electric PM5350",
  "device_type": "tcp",
  "is_connected": true,
  "poll_interval_ms": 1000,
  "last_seen": "2026-11-20T12:00:00Z",
  "consecutive_errors": 0,
  "total_reads": 450,
  "total_errors": 0,
  "retry_count": 0
}
```

### 8.3 Query Historical Telemetry with Pagination
```bash
curl -s -H "X-API-Key: secret-prod-token-2026" \
  "https://rustbridge-iot-platform.onrender.com/api/devices/power-meter-01/history?limit=10&offset=0&register=voltage_a" | jq .
```

**Response:**
```json
{
  "device_id": "power-meter-01",
  "register": "voltage_a",
  "total": 120,
  "limit": 10,
  "offset": 0,
  "records": [
    {
      "device_id": "power-meter-01",
      "register_name": "voltage_a",
      "value": 230.4,
      "raw": [2304],
      "unit": "V",
      "timestamp": "2026-11-20T12:05:00Z"
    }
  ]
}
```

### 8.4 WebSocket Real-Time Stream
Test live WebSocket streaming using `websocat` or any WebSocket client:
```bash
# Connect to stream
websocat wss://rustbridge-iot-platform.onrender.com/ws

# Client sends subscribe command:
{"type":"subscribe","devices":["power-meter-01"]}

# Server streams live register events:
# {"type":"update","data":{"device_id":"power-meter-01","register_name":"voltage_a","value":230.4,"raw":[2304],"unit":"V","timestamp":"2026-11-20T12:05:00Z"}}
```

---

## 9. Prometheus Metrics Verification

Scrape metrics for Grafana or Datadog ingestion:
```bash
curl -s https://rustbridge-iot-platform.onrender.com/metrics | head -n 30
```

**Key Prometheus Metrics:**
```text
# HELP rustbridge_poll_cycles_total Total number of device poll cycles executed
# TYPE rustbridge_poll_cycles_total counter
rustbridge_poll_cycles_total{device="power-meter-01"} 450

# HELP rustbridge_poll_failures_total Total number of device poll failures
# TYPE rustbridge_poll_failures_total counter
rustbridge_poll_failures_total{device="power-meter-01"} 0

# HELP rustbridge_device_status Device connection status (1 = connected, 0 = disconnected)
# TYPE rustbridge_device_status gauge
rustbridge_device_status{device="power-meter-01"} 1

# HELP rustbridge_telemetry_records_ingested_total Total telemetry readings stored
# TYPE rustbridge_telemetry_records_ingested_total counter
rustbridge_telemetry_records_ingested_total 900
```

---

## 10. Local Production Deployment (Docker Compose)

For local demonstration and offline testing without cloud accounts:

### Quick Start with Docker Compose:
```bash
# Start complete stack with Mosquitto MQTT, Modbus Simulator, Prometheus & Grafana
docker compose --profile dev --profile monitoring up -d
```

### Verification:
| Service | URL | Description | Credentials |
| :--- | :--- | :--- | :--- |
| **RustBridge API** | `http://localhost:3000` | REST API & WebSockets | None / `dev-api-key-12345` |
| **Prometheus** | `http://localhost:9090` | Metrics Dashboard | None |
| **Grafana** | `http://localhost:3001` | Visual Dashboards | `admin` / `rustbridge` |
| **Mosquitto MQTT** | `tcp://localhost:1883` | MQTT Broker | Anonymous |
| **Modbus Simulator** | `tcp://localhost:5020` | In-memory Modbus TCP device | Unit ID 1 |

### Stop Stack:
```bash
docker compose down -v
```

---

## 11. Free-Tier Restrictions & Real-World Caveats

When deploying on free cloud infrastructure, be aware of the following architectural constraints:

1. **Auto-Sleep / Inactivity Idle:**
   - On Render's free tier, services spin down after 15 minutes without inbound HTTP traffic.
   - When a request arrives, the service wakes up with a cold start delay of ~45–50 seconds.
   - While asleep, background Modbus polling tasks are paused.
   - *Mitigation:* Use an uptime pinger (e.g., UptimeRobot, Cron-job.org) to ping `/health` every 10 minutes, or deploy on Koyeb / self-hosted VPS.

2. **No Physical RS-485 / Serial Hardware:**
   - Cloud containers do not have physical serial COM ports (`/dev/ttyUSB0` or `COM1`).
   - RTU devices must use an Ethernet-to-Serial gateway (e.g. Moxa NPort) or run in simulated TCP mode.

3. **Ephemeral Container Filesystem:**
   - Local files or SQLite databases written to the container root will be wiped on restart.
   - *Mitigation:* Use managed external PostgreSQL (Neon/Supabase) or In-Memory storage.

4. **Resource Constraints:**
   - 512 MB memory limit: RustBridge is written in Rust with minimal memory footprint (~25 MB runtime resident set size), comfortably fitting within free limits.

---

## 12. Troubleshooting Guide

### Issue: Service returns `502 Bad Gateway` on startup
- **Cause:** Application is still starting up or binding to the wrong port.
- **Fix:** Ensure `RUSTBRIDGE_SERVER_PORT=10000` (on Render) and check logs for binding errors.

### Issue: Modbus device reports `Device disconnected / Read timeout`
- **Cause:** The target IP/port is behind a local NAT or firewall inaccessible from cloud IP ranges.
- **Fix:** Point `RUSTBRIDGE_DEVICE_0_HOST` to a public simulator or deploy RustBridge on an edge device in the same local subnet.

### Issue: Rate limit returns `429 Too Many Requests`
- **Cause:** Client exceeded `rate_limit_rpm` requests per minute.
- **Fix:** Inspect the `Retry-After: 60` HTTP header and back off requests, or adjust `RUSTBRIDGE_SERVER_RATE_LIMIT_RPM`.
