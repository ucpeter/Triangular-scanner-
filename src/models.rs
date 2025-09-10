use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Price for a pair (spot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairPrice {
    pub base: String,
    pub quote: String,
    pub price: f64,
    pub is_spot: bool,
}

/// Triangular arbitrage result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub triangle: String,
    pub pairs: String,
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
}

/// API request body for /api/scan
#[derive(Debug, Clone, Deserialize)]
pub struct ScanRequest {
    pub exchanges: Vec<String>,
    pub min_profit: f64,
}

/// Shared app state kept simple (only last results)
#[derive(Clone)]
pub struct AppState {
    pub last_results: Arc<RwLock<Option<Vec<TriangularResult>>>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            last_results: Arc::new(RwLock::new(None)),
        }
    }
}
