use axum::{routing::post, Json, Router};
use futures::future::join_all;
use serde::Deserialize;
use tracing::info;

use crate::exchanges::collect_exchange_snapshot;
use crate::logic::find_triangular_opportunities;
use crate::models::{PairPrice, TriangularResult};

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

    // Run exchange snapshots in parallel
    let futures = req
        .exchanges
        .iter()
        .map(|exch| {
            let exch = exch.clone();
            async move {
                let pairs: Vec<PairPrice> =
                    collect_exchange_snapshot(&exch, req.collect_seconds).await;
                info!("{}: collected {} pairs", exch, pairs.len());

                let opps = find_triangular_opportunities(
                    &exch,
                    pairs,
                    req.min_profit,
                    0.10,  // fee per leg %
                    100,   // neighbor limit
                );

                info!("{}: found {} opportunities", exch, opps.len());
                opps
            }
        })
        .collect::<Vec<_>>();

    let results_nested = join_all(futures).await;
    let results: Vec<TriangularResult> = results_nested.into_iter().flatten().collect();

    info!("scan complete: {} total opportunities", results.len());

    Json(results)
            }
