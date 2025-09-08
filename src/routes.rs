use axum::{extract::State, response::Json, http::StatusCode};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::models::{AppState, ScanRequest, ScanResponse, ExecMode};
use crate::logic::scan_triangles;

pub async fn ui_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "message": "WS Arbitrage Scanner running",
            "endpoints": {
                "POST /scan": "{ exchanges: [\"binance\"], min_profit: 0.3 }",
                "POST /toggle_live": "\"live\" or \"scan_once\""
            }
        })),
    )
}

pub async fn scan_handler(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(payload): Json<ScanRequest>,
) -> (StatusCode, Json<ScanResponse>) {
    let st = state.lock().await;

    // gather prices from chosen exchanges
    let mut merged: Vec<crate::models::PairPrice> = Vec::new();
    for ex in &payload.exchanges {
        if let Some(v) = st.prices.get(ex) {
            merged.extend_from_slice(v);
        }
    }

    // default fee per leg 0.1%
    let results = scan_triangles(&merged, payload.min_profit, 0.10);

    (
        StatusCode::OK,
        Json(ScanResponse {
            status: "success".to_string(),
            results,
        }),
    )
}

#[derive(serde::Deserialize)]
pub struct TogglePayload {
    pub mode: String, // "live" | "scan_once"
}

pub async fn toggle_handler(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(payload): Json<TogglePayload>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut st = state.lock().await;
    st.mode = match payload.mode.as_str() {
        "live" => ExecMode::Live,
        _ => ExecMode::ScanOnce,
    };

    (
        StatusCode::OK,
        Json(json!({ "status": "mode updated", "mode": payload.mode })),
    )
  }
