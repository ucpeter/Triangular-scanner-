use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Represents one market pair price
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairPrice {
    pub base: String,
    pub quote: String,
    pub price: f64,
    pub is_spot: bool,
}

/// Arbitrage scan result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub path: String,
    pub pairs: String,
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
}

/// Request payload for /scan
#[derive(Debug, Clone, Deserialize)]
pub struct ScanRequest {
    pub exchanges: Vec<String>,
    pub min_profit: f64,
}

/// Shared app state
#[derive(Clone)]
pub struct AppState {
    pub prices: SharedPrices,
}

/// Type alias for shared exchange data
pub type SharedPrices = Arc<RwLock<HashMap<String, Vec<PairPrice>>>>;
