// src/utils.rs
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

/// Round to 2 decimal places
pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Round to 4 decimal places
pub fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Current timestamp in milliseconds
pub fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

/// Uniform log helper for exchanges
pub fn log_pairs(exchange: &str, count: usize) {
    info!("[{}] processed {} pairs", exchange, count);
}
