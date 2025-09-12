use axum::{Json, Router, routing::post};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::models::{PairPrice, TriangularResult};
use crate::exchanges::collect_exchange_snapshot;
use crate::arbitrage::find_triangular_opportunities;

#[derive(Deserialize)]
pub struct ScanRequest {
    exchanges: Vec<String>,
    min_profit: f64,
}

/// POST /scan
pub async fn scan(Json(payload): Json<ScanRequest>) -> Json<serde_json::Value> {
    info!("Received scan request for exchanges: {:?}", payload.exchanges);

    let mut all_pairs: Vec<PairPrice> = Vec::new();

    for ex in &payload.exchanges {
        let snapshot = collect_exchange_snapshot(ex, 5).await; // collect for 5s per exchange
        all_pairs.extend(snapshot);
    }

    let results: Vec<TriangularResult> =
        find_triangular_opportunities(&all_pairs, payload.min_profit);

    Json(json!({
        "status": "ok",
        "count": results.len(),
        "results": results
    }))
}

/// Build router
pub fn routes() -> Router {
    Router::new()
        .route("/scan", post(scan))
}
