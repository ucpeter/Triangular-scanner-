use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairPrice {
    pub base: String,
    pub quote: String,
    pub price: f64,
    pub is_spot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub triangle: String,
    pub profit_before_fees: f64,
    pub trade_fees: f64,
    pub profit_after_fees: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScanRequest {
    pub exchanges: Vec<String>,
    pub min_profit: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanResponse {
    pub status: String,
    pub count: usize,
    pub results: Vec<TriangularResult>,
}

#[derive(Debug, Clone)]
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
