use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairPrice {
    pub base: String,
    pub quote: String,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub triangle: String,
    pub pairs: String,
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScanRequest {
    pub exchanges: Vec<String>,
    pub min_profit: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TogglePayload {
    pub mode: String, // "live" or "scan_once"
}

#[derive(Debug, Clone, Copy)]
pub enum ExecMode {
    ScanOnce,
    Live,
}

impl Default for ExecMode {
    fn default() -> Self {
        ExecMode::ScanOnce
    }
}

/// Shared app state (kept minimal).
#[derive(Clone)]
pub struct AppState {
    /// Execution mode (scan once or live)
    pub mode: Arc<RwLock<ExecMode>>,
    /// Last scan results (optional)
    pub last_results: Arc<RwLock<Option<Vec<TriangularResult>>>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            mode: Arc::new(RwLock::new(ExecMode::default())),
            last_results: Arc::new(RwLock::new(None)),
        }
    }
}
