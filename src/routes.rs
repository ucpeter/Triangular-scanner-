use axum::{extract::State, response::Json, http::StatusCode};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::{AppState, ScanRequest, ScanResponse, TriangularResult};
use crate::logic::scan_triangles;
use crate::ws_manager::gather_prices_for_exchanges;

pub async fn ui_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "message": "Triangular Arbitrage Scanner (WS)",
            "usage": "POST /scan with { exchanges: [], min_profit: number }"
        })),
    )
}

pub async fn scan_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let prices = match gather_prices_for_exchanges(&payload.exchanges).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "status": "error", "message": e })),
            );
        }
    };

    let results: Vec<TriangularResult> = scan_triangles(&prices, payload.min_profit, 0.1);

    {
        let mut guard = state.last_results.write().await;
        *guard = Some(results.clone());
    }

    (
        StatusCode::OK,
        Json(json!(ScanResponse {
            status: "success".to_string(),
            count: results.len(),
            results
        })),
    )
        }
