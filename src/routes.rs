use axum::{extract::Query, Json};
use serde::Deserialize;
use crate::models::{TriangularResult, PairPrice};
use crate::logic::find_triangular_opportunities;
use crate::exchanges::collect_exchange_snapshot;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct ScanQuery {
    pub exchange: Option<String>,
    pub min_profit: Option<f64>,
    pub collect_seconds: Option<u64>,
}

pub async fn scan_handler(Query(params): Query<ScanQuery>) -> Json<Vec<TriangularResult>> {
    let exchange = params.exchange.unwrap_or_else(|| "binance".to_string());
    let min_profit = params.min_profit.unwrap_or(0.3);
    let collect_window = params.collect_seconds.unwrap_or(2);

    info!(
        "Received scan request exchange={} min_profit={} collect_seconds={}",
        exchange, min_profit, collect_window
    );

    // 1. fetch snapshot (using WS wrapper you already have)
    let pairs: Vec<PairPrice> = collect_exchange_snapshot(&exchange, collect_window).await;

    // 2. run triangular arbitrage logic
    let results = find_triangular_opportunities(&pairs, min_profit);

    Json(results)
}
