// src/routes.rs
use axum::{extract::State, response::Json, http::StatusCode};
use serde_json::json;
use std::sync::Arc;
use tokio::time::timeout;
use std::time::Duration;
use crate::models::{ScanRequest, PairPrice, ArbResult};
use crate::{exchanges, logic};

type AppState = ();

pub async fn ui_handler() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({"message":"Triangular WS scan-on-demand API running"})))
}

pub async fn scan_handler(
    State(_state): State<Arc<AppState>>,
    axum::extract::Json(payload): axum::extract::Json<ScanRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let collect_seconds = payload.collect_seconds.unwrap_or(2);
    let min_profit = payload.min_profit;
    let fee_per_leg = 0.10_f64; // default per-leg fee (0.1%)

    // spawn fetch tasks
    let mut handles = vec![];
    for ex in payload.exchanges.iter() {
        let ex_lower = ex.to_lowercase();
        let collect = collect_seconds;
        match ex_lower.as_str() {
            "binance" => {
                handles.push(tokio::spawn(async move {
                    match timeout(Duration::from_secs(20), exchanges::fetch_binance(Some(collect))).await {
                        Ok(Ok(v)) => Ok(v),
                        Ok(Err(e)) => Err(format!("binance err: {}", e)),
                        Err(_) => Err("binance timeout".to_string()),
                    }
                }));
            }
            "bybit" => {
                handles.push(tokio::spawn(async move {
                    match timeout(Duration::from_secs(20), exchanges::fetch_bybit(Some(collect))).await {
                        Ok(Ok(v)) => Ok(v),
                        Ok(Err(e)) => Err(format!("bybit err: {}", e)),
                        Err(_) => Err("bybit timeout".to_string()),
                    }
                }));
            }
            "kucoin" => {
                handles.push(tokio::spawn(async move {
                    match timeout(Duration::from_secs(25), exchanges::fetch_kucoin(Some(collect))).await {
                        Ok(Ok(v)) => Ok(v),
                        Ok(Err(e)) => Err(format!("kucoin err: {}", e)),
                        Err(_) => Err("kucoin timeout".to_string()),
                    }
                }));
            }
            "gate" | "gateio" => {
                handles.push(tokio::spawn(async move {
                    match timeout(Duration::from_secs(20), exchanges::fetch_gateio(Some(collect))).await {
                        Ok(Ok(v)) => Ok(v),
                        Ok(Err(e)) => Err(format!("gateio err: {}", e)),
                        Err(_) => Err("gateio timeout".to_string()),
                    }
                }));
            }
            other => {
                return (StatusCode::BAD_REQUEST, Json(json!({"status":"error","message": format!("unsupported exchange: {}", other)})));
            }
        }
    }

    // gather results
    let mut merged_pairs: Vec<PairPrice> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for h in handles {
        match h.await {
            Ok(Ok(vec_pairs)) => merged_pairs.extend(vec_pairs),
            Ok(Err(e)) => errors.push(e),
            Err(e) => errors.push(format!("task join err: {}", e)),
        }
    }

    // build price map and run scan
    let price_map = logic::build_price_map(&merged_pairs);
    let results: Vec<ArbResult> = logic::scan_triangles(&price_map, min_profit, fee_per_leg);

    (StatusCode::OK, Json(json!({
        "status": "ok",
        "errors": errors,
        "count_pairs": merged_pairs.len(),
        "count_opportunities": results.len(),
        "results": results
    })))
        }
