use axum::{routing::post, Json, Router};
use serde::Deserialize;
use tracing::{info, warn};
use futures::future::join_all;

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
    #[serde(default = "default_fee")]
    fee_per_leg_pct: f64,
    #[serde(default = "default_neighbor_limit")]
    neighbor_limit: usize,
}

fn default_fee() -> f64 {
    0.10
}
fn default_neighbor_limit() -> usize {
    100
}

async fn scan_handler(Json(req): Json<ScanRequest>) -> Json<Vec<TriangularResult>> {
    if req.min_profit < 0.0 || req.collect_seconds == 0 {
        warn!("Invalid request: {:?}", req);
        return Json(Vec::new());
    }

    info!(
        "scan request: exchanges={:?} min_profit={} collect_seconds={}",
        req.exchanges, req.min_profit, req.collect_seconds
    );

    let futures: Vec<_> = req
        .exchanges
        .iter()
        .map(|exch| collect_exchange_snapshot(exch, req.collect_seconds))
        .collect();

    let mut results: Vec<TriangularResult> = Vec::new();

    let snapshots: Vec<Vec<PairPrice>> = join_all(futures).await;

    for (exch, pairs) in req.exchanges.iter().zip(snapshots.into_iter()) {
        info!("{}: collected {} pairs", exch, pairs.len());

        let opps = find_triangular_opportunities(
            exch,
            pairs,
            req.min_profit,
            req.fee_per_leg_pct,
            req.neighbor_limit,
        );
        let count = opps.len();
        results.extend(opps);

        info!("{}: found {} opportunities", exch, count);
    }

    info!("scan complete: {} total opportunities", results.len());

    Json(results)
    }
