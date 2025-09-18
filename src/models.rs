// src/models.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairPrice {
    pub base: String,
    pub quote: String,
    pub price: f64,
    pub is_spot: bool,
    pub volume: f64, // reported volume (useful as liquidity metric)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub triangle: String,       // e.g. "A -> B -> C -> A"
    pub pairs: Vec<String>,     // e.g. ["A/B", "B/C", "C/A"]
    pub profit_before: f64,     // %
    pub fees: f64,              // total fees percent
    pub profit_after: f64,      // % after fees
    pub score_liquidity: f64,   // aggregated liquidity score (for sorting / prioritisation)
}
