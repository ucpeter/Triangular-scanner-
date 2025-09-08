use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    // Execution mode: "scan" | "live" (handled server-side)
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanResponse {
    pub status: String,
    pub results: Vec<TriangularResult>,
}

#[derive(Debug)]
pub enum ExecMode {
    ScanOnce,
    Live,
}

#[derive(Debug, Default)]
pub struct AppState {
    /// map exchange -> map symbol -> PairPrice (store as vector to make sharing simple)
    /// We store Vec<PairPrice> per exchange for compatibility with logic.
    pub prices: HashMap<String, Vec<PairPrice>>,
    pub mode: ExecMode,
}
