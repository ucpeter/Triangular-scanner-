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
use crate::ws_manager;

type SharedAppState = Arc<TokioRwLock<AppState>>;

/// UI endpoint
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
    axum::extract::Json(payload): axum::extract::Json<ScanRequest>,
) -> impl IntoResponse {
    // gather live prices via ws_manager helper
    let merged = match ws_manager::gather_prices_for_exchanges(&payload.exchanges).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status":"error","message": format!("failed to gather prices: {}", e)})),
            );
        }
    };

    // default fee per leg 0.10%
    let results = scan_triangles(&merged, payload.min_profit, 0.10);

    // store last results
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
        // if AppState has an exec_mode field, set it here — adjust AppState accordingly in models.rs
        // Example: guard.exec_mode = mode;
        // For now we just keep last_results in state; add exec_mode if you want toggle to change runtime behaviour.
        let _ = &guard; // no-op to avoid unused variable warning
    }

    (
        StatusCode::OK,
        Json(json!({"status":"ok","mode": payload.mode})),
    )
            }
