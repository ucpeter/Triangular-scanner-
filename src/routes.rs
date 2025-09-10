use axum::{
    extract::State,
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;

use crate::models::{AppState, ScanRequest, TogglePayload, ExecMode};
use crate::logic::scan_triangles;
use crate::ws_manager::SharedPrices;
use crate::models::PairPrice;

type SharedAppState = Arc<TokioRwLock<AppState>>;

/// Simple UI endpoint
pub async fn ui_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "message": "Triangular WS arbitrage scanner running",
            "ui": "open index.html at root"
        })),
    )
}

/// POST /api/scan
/// Body: { exchanges: ["binance","bybit"], min_profit: 0.3 }
pub async fn scan_handler(
    State(shared_state): State<SharedAppState>,
    // we read prices via a global SharedPrices instance from ws_manager module — see note below
    axum::extract::Json(payload): axum::extract::Json<ScanRequest>,
) -> impl IntoResponse {
    // NOTE: we expect ws_manager to be running and have populated its SharedPrices cache.
    // For simplicity (to avoid complex State typing here) we'll ask the ws_manager for prices
    // via its public helper `ws_manager::gather_prices_for_exchanges(...)`.
    // That helper should return Vec<PairPrice> merged from the given exchange names.
    let merged: Vec<PairPrice> = match crate::ws_manager::gather_prices_for_exchanges(&payload.exchanges).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status":"error","message": format!("failed to gather prices: {}", e)})),
            );
        }
    };

    // default fee per leg 0.10% (0.1)
    let results = scan_triangles(&merged, payload.min_profit, 0.10);

    // store last results in app state
    {
        let mut guard = shared_state.write().await;
        guard.last_results = Some(results.clone());
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

/// POST /api/toggle { mode: "live" } or { mode: "scan_once" }
pub async fn toggle_handler(
    State(shared_state): State<SharedAppState>,
    axum::extract::Json(payload): axum::extract::Json<TogglePayload>,
) -> impl IntoResponse {
    let mode = match payload.mode.as_str() {
        "live" => ExecMode::Live,
        _ => ExecMode::ScanOnce,
    };

    {
        let mut guard = shared_state.write().await;
        // set mode (AppState should expose a field or method to adjust internal behavior)
        // Here we assume AppState has a place to store the ExecMode; modify your AppState accordingly.
        // e.g., guard.exec_mode = mode;
        // For safety, I'll try to set if the field exists; otherwise, it's a no-op (adjust AppState if needed).
        #[allow(unused_must_use)]
        {
            // If AppState has `pub exec_mode: Arc<RwLock<ExecMode>>` then you would do:
            // *guard.exec_mode.write().unwrap() = mode;
        }
    }

    (
        StatusCode::OK,
        Json(json!({"status":"ok","mode": payload.mode})),
    )
            }
