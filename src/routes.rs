use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use tracing::info;

use crate::models::{AppState, ScanRequest, TriangularResult};

/// Dummy scan logic (replace with real triangular arbitrage calculation)
async fn run_scan(
    exchanges: Vec<String>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    info!("scanning exchanges={:?} min_profit={}", exchanges, min_profit);

    // TODO: plug in real arbitrage logic
    vec![TriangularResult {
        path: "USDT → BTC → ETH → USDT".to_string(),
        pairs: "USDT/BTC | BTC/ETH | ETH/USDT".to_string(),
        profit_before: 1.23,
        fees: 0.05,
        profit_after: 1.18,
    }]
}

/// POST /scan
pub async fn scan_handler(
    State(state): State<AppState>,
    Json(payload): Json<ScanRequest>,
) -> impl IntoResponse {
    info!("received scan request: {:?}", payload);

    let results = run_scan(payload.exchanges, payload.min_profit).await;

    Json(results)
}
