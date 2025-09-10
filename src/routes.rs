use axum::{extract::State, http::StatusCode, response::Json};
use serde_json::json;
use std::sync::Arc;
use crate::models::{AppState, ScanRequest, TriangularResult};
use crate::ws_manager;
use crate::logic;

pub async fn ui_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "message": "Triangular WS Arbitrage Scanner running",
            "usage": "POST /api/scan with { exchanges: [], min_profit: number }"
        })),
    )
}

pub async fn scan_handler(
    State(app_state): State<Arc<AppState>>,
    axum::extract::Json(payload): axum::extract::Json<ScanRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // collect current prices for requested exchanges
    let merged = match ws_manager::gather_prices_for_exchanges(&payload.exchanges).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status":"error","message": format!("gather prices failed: {}", e)})),
            );
        }
    };

    // default fee per leg 0.10%
    let fee_per_leg = 0.10;
    let results: Vec<TriangularResult> = logic::scan_triangles(&merged, payload.min_profit, fee_per_leg);

    // store last results
    {
        let mut guard = app_state.last_results.write().await;
        *guard = Some(results.clone());
    }

    (
        StatusCode::OK,
        Json(json!({
            "status":"success",
            "count": results.len(),
            "results": results
        })),
    )
}
