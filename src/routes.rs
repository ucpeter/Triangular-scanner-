use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::{AppState, ScanRequest, TriangularResult};
use crate::ws_manager::gather_prices_for_exchanges;

pub async fn ui_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "message": "Triangular Arbitrage Scanner (WS) is running",
            "usage": "POST /scan with { exchanges: [], min_profit: number }"
        })),
    )
}

pub async fn scan_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Fetch latest prices from selected exchanges (via WS manager)
    let merged: Vec<crate::models::PairPrice> =
        match gather_prices_for_exchanges(&payload.exchanges).await {
            Ok(data) => data,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "status": "error",
                        "message": format!("Failed to fetch prices: {}", e),
                    })),
                )
            }
        };

    // Run arbitrage scan
    let results: Vec<TriangularResult> =
        crate::logic::scan_triangles(&merged, payload.min_profit, 0.1);

    // ✅ Store results in shared state
    {
        let mut lr = state.last_results.write().await;
        *lr = Some(results.clone());
    }

    (
        StatusCode::OK,
        Json(json!({
            "status": "success",
            "count": results.len(),
            "results": results,
        })),
    )
            }
