use serde::{Deserialize, Serialize};

/// Represents a trading pair price snapshot from an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairPrice {
    pub base: String,
    pub quote: String,
    pub price: f64,
    pub is_spot: bool,
    pub volume: f64,
}

/// Result of a detected triangular arbitrage opportunity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub triangle: String,
    pub pairs: Vec<String>,
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
    pub score_liquidity: f64,
    pub liquidity_legs: [f64; 3],   // NEW
}
