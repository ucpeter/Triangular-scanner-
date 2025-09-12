use axum::{Json, Router, routing::post};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::models::{PairPrice, TriangularResult};
use crate::exchanges::collect_exchange_snapshot;
use crate::logic::find_triangular_opportunities;

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub exchanges: Vec<String>,
    pub min_profit: Option<f64>,
    // optional: allow custom collect window per request if you want
    pub collect_seconds: Option<u64>,
}

pub async fn scan_handler(Json(payload): Json<ScanRequest>) -> Json<serde_json::Value> {
    let exchanges = if payload.exchanges.is_empty() {
        vec!["binance".to_string()]
    } else {
        payload.exchanges
    };

    let min_profit = payload.min_profit.unwrap_or(0.0);
    // default collect window = 2 seconds if not provided
    let collect_seconds = payload.collect_seconds.unwrap_or(2);

    info!(
        "scan request: exchanges={:?} min_profit={} collect_seconds={}",
        exchanges, min_profit, collect_seconds
    );

    // collect snapshots sequentially (keeps logic simple)
    let mut all_pairs: Vec<PairPrice> = Vec::new();
    for ex in &exchanges {
        let snapshot: Vec<PairPrice> = collect_exchange_snapshot(ex, collect_seconds).await;
        all_pairs.extend(snapshot);
    }

    // run triangular arbitrage logic (expects a function in logic.rs)
    let results: Vec<TriangularResult> = find_triangular_opportunities(&all_pairs, min_profit);

    Json(json!({
        "status": "ok",
        "count": results.len(),
        "results": results
    }))
}

/// Build and return the router for this module
pub fn routes() -> Router {
    Router::new().route("/scan", post(scan_handler))
        }
