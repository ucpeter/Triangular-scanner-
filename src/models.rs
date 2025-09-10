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
    pub pairs: String,
    pub profit_before_fees: f64,
    pub fees: f64,
    pub profit_after_fees: f64,
}

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub exchanges: Vec<String>,
    pub min_profit: f64,
}

#[derive(Clone)]
pub struct AppState {
    // ✅ last_results is just wrapped in an RwLock for thread safety
    pub last_results: Arc<RwLock<Option<Vec<TriangularResult>>>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            last_results: Arc::new(RwLock::new(None)),
        }
    }
        }
