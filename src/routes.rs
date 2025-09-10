use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;
use crate::models::{AppState, ScanRequest, TogglePayload, ExecMode};
use crate::logic::scan_triangles;
use crate::ws_manager::SharedPrices;

type SharedAppState = Arc<tokio::sync::RwLock<AppState>>;

pub async fn router() -> axum::Router {
    axum::Router::new()
        .route("/", axum::routing::get(ui_handler))
        .route("/scan", axum::routing::post(scan_handler))
        .route("/toggle", axum::routing::post(toggle_handler))
}

pub async fn ui_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "message":"Triangular WS arbitrage scanner running",
            "ui":"open index.html on site root"
        })),
    )
}

/// POST /scan
/// Body: { exchanges: ["binance","bybit"], min_profit: 0.3 }
pub async fn scan_handler(
    State(app_state): State<AppStateExtractor>,
    State(prices_map): State<SharedPricesWrapper>,
    axum::extract::Json(payload): axum::extract::Json<ScanRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // gather selected exchanges' prices
    let mut merged: Vec<crate::models::PairPrice> = Vec::new();
    {
        let guard = prices_map.0.read().await;
        for ex in &payload.exchanges {
            if let Some(vec) = guard.get(ex) {
                merged.extend_from_slice(vec);
            }
        }
    }

    // default fee per leg 0.1%
    let results = scan_triangles(&merged, payload.min_profit, 0.10);

    // store last results
    {
        let mut lr = app_state.0.last_results.write().unwrap();
        *lr = Some(results.clone());
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

/// Toggle POST /toggle { mode: "live" } or { mode: "scan_once" }
pub async fn toggle_handler(
    State(app_state): State<AppStateExtractor>,
    axum::extract::Json(payload): axum::extract::Json<TogglePayload>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mode = match payload.mode.as_str() {
        "live" => ExecMode::Live,
        _ => ExecMode::ScanOnce,
    };

    {
        let mut guard = app_state.0.mode.write().unwrap();
        *guard = mode;
    }

    (
        StatusCode::OK,
        Json(json!({"status":"ok","mode": payload.mode})),
    )
}

/// Tiny wrappers used to allow axum State with different shared types
pub struct AppStateExtractor(pub AppState);
pub struct SharedPricesWrapper(pub SharedPrices);

impl State<AppStateExtractor> for AppStateExtractor {
    fn from_request_parts<B>(_parts: &mut axum::http::request::Parts, _state: &AppStateExtractor) -> std::future::Ready<Result<Self, axum::Error>> {
        std::future::ready(Err(axum::Error::from(std::io::Error::new(std::io::ErrorKind::Other, "not used"))))
    }
        }
