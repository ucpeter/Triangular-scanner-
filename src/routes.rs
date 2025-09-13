use axum::{
    extract::Json,
    http::StatusCode,
    response::Response,
    routing::post,
    Router,
};
use serde::Deserialize;
use tracing::info;

use crate::exchanges::collect_exchange_snapshot;
use crate::logic::{find_triangular_opportunities, TriangularResult};
use crate::models::PairPrice;
use axum::body::Body;

pub fn routes() -> Router {
    Router::new().route("/scan", post(scan_handler))
}

#[derive(Debug, Deserialize)]
struct ScanRequest {
    exchanges: Vec<String>,
    min_profit: f64,
    collect_seconds: u64,
}

async fn scan_handler(Json(req): Json<ScanRequest>) -> Response {
    info!(
        "scan request: exchanges={:?} min_profit={} collect_seconds={}",
        req.exchanges, req.min_profit, req.collect_seconds
    );

    let mut results: Vec<TriangularResult> = Vec::new();

    for exch in req.exchanges {
        let pairs: Vec<PairPrice> = collect_exchange_snapshot(&exch, req.collect_seconds).await;
        let opps = find_triangular_opportunities(&exch, pairs, req.min_profit);
        results.extend(opps);
    }

    let body = match serde_json::to_string(&results) {
        Ok(json) => json,
        Err(e) => {
            tracing::error!("failed to serialize results: {:?}", e);
            "[]".to_string()
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}
