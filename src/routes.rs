use axum::{
    routing::post,
    Json, Router,
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use tracing::info;

use crate::exchanges::collect_exchange_snapshot;
use crate::logic::{find_triangular_opportunities, TriangularResult};
use crate::models::PairPrice;

pub fn routes() -> Router {
    Router::new().route("/scan", post(scan_handler))
}

#[derive(Debug, Deserialize)]
struct ScanRequest {
    exchanges: Vec<String>,
    min_profit: f64,
    collect_seconds: u64,
}

async fn scan_handler(Json(req): Json<ScanRequest>) -> impl IntoResponse {
    info!(
        "scan request: exchanges={:?} min_profit={} collect_seconds={}",
        req.exchanges, req.min_profit, req.collect_seconds
    );

    let mut results: Vec<TriangularResult> = Vec::new();

    for exch in req.exchanges {
        let pairs: Vec<PairPrice> = collect_exchange_snapshot(&exch, req.collect_seconds).await;
        info!("{} collected {} pairs", exch, pairs.len());

        let opps = find_triangular_opportunities(&exch, pairs, req.min_profit);
        info!("{} opportunities found: {}", exch, opps.len());

        results.extend(opps);
    }

    // Always return JSON, even if empty
    (StatusCode::OK, Json(results))
}
