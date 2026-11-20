//! Device Connection and Retry Logic Tests
//!
//! Validates:
//! - Exponential backoff calculation and jitter bounds
//! - Minimum and maximum backoff clamps
//! - Error accumulation and consecutive error limits
//! - Recovery and status transitions

use rustbridge::modbus::calculate_backoff;
use rustbridge::storage::{DeviceRecord, InMemoryStorage};
use std::time::Duration;

#[test]
fn test_exponential_backoff_progression() {
    let base = 500;
    let factor = 2.0;
    let max = 15000;

    // Retry 1: ~base (500ms ± 15%)
    let d1 = calculate_backoff(1, base, max, factor);
    assert!(d1 >= Duration::from_millis(425));
    assert!(d1 <= Duration::from_millis(575));

    // Retry 2: ~base * 2 = 1000ms ± 15%
    let d2 = calculate_backoff(2, base, max, factor);
    assert!(d2 >= Duration::from_millis(850));
    assert!(d2 <= Duration::from_millis(1150));

    // Retry 3: ~base * 4 = 2000ms ± 15%
    let d3 = calculate_backoff(3, base, max, factor);
    assert!(d3 >= Duration::from_millis(1700));
    assert!(d3 <= Duration::from_millis(2300));
}

#[test]
fn test_backoff_clamped_to_max() {
    let base = 1000;
    let factor = 2.0;
    let max = 5000;

    // Retry 10: 1000 * 2^9 = 512,000ms, should clamp to max (5000ms ± 15%)
    let d = calculate_backoff(10, base, max, factor);
    assert!(d <= Duration::from_millis(5750));
    assert!(d >= Duration::from_millis(4250));
}

#[test]
fn test_backoff_zero_attempt_returns_min() {
    let d = calculate_backoff(0, 1000, 5000, 2.0);
    assert_eq!(d, Duration::from_millis(1000));
}

#[tokio::test]
async fn test_device_status_transitions_and_recovery() {
    let storage = InMemoryStorage::new(100);
    let dev = DeviceRecord::new("boiler-01", "Industrial Boiler", "tcp", 1000);
    storage.save_device(&dev).await.unwrap();

    // Initial state: not connected
    let d = storage.get_device("boiler-01").await.unwrap().unwrap();
    assert!(!d.is_connected);
    assert_eq!(d.consecutive_errors, 0);

    // Polling succeeds
    storage
        .update_device_status("boiler-01", true, true, false)
        .await
        .unwrap();
    let d = storage.get_device("boiler-01").await.unwrap().unwrap();
    assert!(d.is_connected);
    assert_eq!(d.total_reads, 1);
    assert_eq!(d.consecutive_errors, 0);

    // Polling failure occurs
    storage
        .update_device_status("boiler-01", false, false, false)
        .await
        .unwrap();
    let d = storage.get_device("boiler-01").await.unwrap().unwrap();
    assert!(!d.is_connected);
    assert_eq!(d.total_errors, 1);
    assert_eq!(d.consecutive_errors, 1);

    // Second failure occurs with retry
    storage
        .update_device_status("boiler-01", false, false, true)
        .await
        .unwrap();
    let d = storage.get_device("boiler-01").await.unwrap().unwrap();
    assert!(!d.is_connected);
    assert_eq!(d.total_errors, 2);
    assert_eq!(d.consecutive_errors, 2);
    assert_eq!(d.retry_count, 1);

    // Device recovers on next poll cycle
    storage
        .update_device_status("boiler-01", true, true, false)
        .await
        .unwrap();
    let d = storage.get_device("boiler-01").await.unwrap().unwrap();
    assert!(d.is_connected);
    assert_eq!(d.total_reads, 2);
    assert_eq!(d.consecutive_errors, 0); // Consecutive errors reset to 0
    assert_eq!(d.total_errors, 2); // Historical total preserved
}
