use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairPrice {
    pub base: String,
    pub quote: String,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub route: String,
    pub start_amount: f64,
    pub end_amount: f64,
    pub profit_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecMode {
    Live,   // continuous updates from WS
    OnScan, // only run when user requests
}

#[derive(Clone)]
pub struct AppState {
    pub mode: Arc<RwLock<ExecMode>>,
    pub pairs: Arc<RwLock<Vec<PairPrice>>>,
}
