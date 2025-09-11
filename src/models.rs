// src/models.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairPrice {
    pub base: String,
    pub quote: String,
    pub price: f64,
    pub is_spot: bool,
}

pub type PriceMap = HashMap<String, HashMap<String, f64>>;

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub exchanges: Vec<String>,
    pub min_profit: f64, // percent e.g. 0.3
    pub collect_seconds: Option<u64>, // optional seconds to collect WS tickers
}

#[derive(Debug, Serialize)]
pub struct ArbResult {
    pub route: String,
    pub pairs: String,
    pub profit_before: f64,
    pub fee_percent: f64,
    pub profit_after: f64,
}
