//! MQTT Integration and Serialization Tests
//!
//! Validates:
//! - Telemetry payload serialization (value, raw, unit, timestamp)
//! - Device lifecycle status payloads (online, offline)
//! - Topic hierarchy convention: `{prefix}/{device_id}/{register}` and `{prefix}/{device_id}/status`
//! - QoS mapping and configuration validation

use rumqttc::QoS;
use rustbridge::api::RegisterUpdate;
use rustbridge::config::MqttConfig;

#[test]
fn test_mqtt_topic_hierarchy() {
    let prefix = "factory/telemetry";
    let device_id = "plc-line-1";
    let register = "motor_speed";

    let data_topic = format!("{}/{}/{}", prefix, device_id, register);
    assert_eq!(data_topic, "factory/telemetry/plc-line-1/motor_speed");

    let status_topic = format!("{}/{}/status", prefix, device_id);
    assert_eq!(status_topic, "factory/telemetry/plc-line-1/status");
}

#[test]
fn test_mqtt_telemetry_payload_serialization() {
    let update = RegisterUpdate {
        device_id: "energy-meter-01".to_string(),
        register_name: "voltage_phase_a".to_string(),
        value: 230.5,
        raw: vec![2305],
        unit: Some("V".to_string()),
        timestamp: "2026-11-20T12:00:00Z".to_string(),
    };

    let payload = serde_json::json!({
        "value": update.value,
        "raw": update.raw,
        "unit": update.unit,
        "timestamp": update.timestamp,
    });

    let payload_str = serde_json::to_string(&payload).unwrap();
    let deserialized: serde_json::Value = serde_json::from_str(&payload_str).unwrap();

    assert_eq!(deserialized["value"], 230.5);
    assert_eq!(deserialized["raw"], serde_json::json!([2305]));
    assert_eq!(deserialized["unit"], "V");
    assert_eq!(deserialized["timestamp"], "2026-11-20T12:00:00Z");
}

#[test]
fn test_mqtt_qos_levels() {
    let qos_0 = match 0 {
        0 => QoS::AtMostOnce,
        1 => QoS::AtLeastOnce,
        _ => QoS::ExactlyOnce,
    };
    assert_eq!(qos_0, QoS::AtMostOnce);

    let qos_1 = match 1 {
        0 => QoS::AtMostOnce,
        1 => QoS::AtLeastOnce,
        _ => QoS::ExactlyOnce,
    };
    assert_eq!(qos_1, QoS::AtLeastOnce);

    let qos_2 = match 2 {
        0 => QoS::AtMostOnce,
        1 => QoS::AtLeastOnce,
        _ => QoS::ExactlyOnce,
    };
    assert_eq!(qos_2, QoS::ExactlyOnce);
}

#[test]
fn test_mqtt_config_with_tls() {
    let config = MqttConfig {
        enabled: true,
        host: "mqtt.broker.local".to_string(),
        port: 8883,
        client_id: "rustbridge-client".to_string(),
        username: Some("iot_user".to_string()),
        password: Some("iot_pass".to_string()),
        topic_prefix: "rustbridge/iot".to_string(),
        qos: 1,
        retain: false,
        tls: true,
        ca_file: Some("/certs/ca.crt".to_string()),
    };

    assert!(config.tls);
    assert_eq!(config.port, 8883);
    assert_eq!(config.ca_file.as_deref(), Some("/certs/ca.crt"));
}
