use axum::{routing::post, Json, Router};
use serde::Deserialize;
use tracing::info;

use crate::exchanges::collect_exchange_snapshot;
use crate::logic::find_triangular_opportunities;
use crate::models::{TriangularResult, PairPrice};

pub fn routes() -> Router {
    Router::new().route("/scan", post(scan_handler))
}

#[derive(Debug, Deserialize)]
struct ScanRequest {
    exchanges: Vec<String>,
    min_profit: f64,
    collect_seconds: u64,
}

async fn scan_handler(Json(req): Json<ScanRequest>) -> Json<Vec<TriangularResult>> {
    info!(
        "scan request: exchanges={:?} min_profit={} collect_seconds={}",
        req.exchanges, req.min_profit, req.collect_seconds
    );

    let mut results: Vec<TriangularResult> = Vec::new();

    for exch in req.exchanges {
        let pairs: Vec<PairPrice> = collect_exchange_snapshot(&exch, req.collect_seconds).await;
        let opps = find_triangular_opportunities(&exch, pairs, req.min_profit);

        // convert to owned values if opps is Vec<&TriangularResult>
        results.extend(opps.into_iter().cloned());
    }

    Json(results)
    }
