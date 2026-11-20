-- =============================================================================
-- Migration: 20261120000001_create_telemetry_tables.sql
-- Project: RustBridge IoT Platform
-- Author: Dhanya
-- Purpose: Schema for Device Metadata and Historical Telemetry Persistence
-- =============================================================================

-- Table: devices
-- Stores registered Modbus devices, connection configuration, and status
CREATE TABLE IF NOT EXISTS devices (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    device_type VARCHAR(32) NOT NULL,
    is_connected BOOLEAN NOT NULL DEFAULT FALSE,
    poll_interval_ms BIGINT NOT NULL DEFAULT 1000,
    last_seen TIMESTAMPTZ,
    consecutive_errors INT NOT NULL DEFAULT 0,
    total_reads BIGINT NOT NULL DEFAULT 0,
    total_errors BIGINT NOT NULL DEFAULT 0,
    retry_count BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Table: telemetry_history
-- Stores historical register readings ingested from polled Modbus devices
CREATE TABLE IF NOT EXISTS telemetry_history (
    id BIGSERIAL PRIMARY KEY,
    device_id VARCHAR(64) NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    register_name VARCHAR(64) NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    raw_values INTEGER[] NOT NULL,
    unit VARCHAR(32),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indices for rapid querying and time-series pagination
CREATE INDEX IF NOT EXISTS idx_telemetry_device_time 
    ON telemetry_history (device_id, recorded_at DESC);

CREATE INDEX IF NOT EXISTS idx_telemetry_device_register_time 
    ON telemetry_history (device_id, register_name, recorded_at DESC);

CREATE INDEX IF NOT EXISTS idx_devices_connected 
    ON devices (is_connected);
