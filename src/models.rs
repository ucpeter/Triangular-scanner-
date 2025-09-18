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
    /// Human-readable triangle representation: A → B → C → A
    pub triangle: String,

    /// The three trading pairs involved, formatted as ["A/B", "B/C", "C/A"]
    pub pairs: Vec<String>,

    /// Gross profit before fees, as percentage.
    pub profit_before: f64,

    /// Total fees applied (sum of per-leg fees).
    pub fees: f64,

    /// Net profit after applying fees, as percentage.
    pub profit_after: f64,

    /// Liquidity score, usually min(volume across 3 legs).
    pub score_liquidity: f64,
}
