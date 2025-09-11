// src/routes.rs
use axum::{extract::State, response::Json, http::StatusCode};
use serde_json::json;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
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
    let mut tasks = vec![];
    for ex in payload.exchanges.iter() {
        match ex.to_lowercase().as_str() {
            "binance" => {
                tasks.push(tokio::spawn(async move {
                    let res = timeout(Duration::from_secs(15), exchanges::fetch_binance(Some(collect_seconds))).await;
                    match res {
                        Ok(Ok(v)) => Ok(("binance".to_string(), v)),
                        Ok(Err(e)) => Err(format!("binance err: {}", e)),
                        Err(_) => Err("binance timeout".to_string()),
                    }
                }));
            }
            "bybit" => {
                tasks.push(tokio::spawn(async move {
                    let res = timeout(Duration::from_secs(15), exchanges::fetch_bybit(Some(collect_seconds))).await;
                    match res {
                        Ok(Ok(v)) => Ok(("bybit".to_string(), v)),
                        Ok(Err(e)) => Err(format!("bybit err: {}", e)),
                        Err(_) => Err("bybit timeout".to_string()),
                    }
                }));
            }
            "kucoin" => {
                tasks.push(tokio::spawn(async move {
                    let res = timeout(Duration::from_secs(20), exchanges::fetch_kucoin(Some(collect_seconds))).await;
                    match res {
                        Ok(Ok(v)) => Ok(("kucoin".to_string(), v)),
                        Ok(Err(e)) => Err(format!("kucoin err: {}", e)),
                        Err(_) => Err("kucoin timeout".to_string()),
                    }
                }));
            }
            "gate" | "gateio" => {
                tasks.push(tokio::spawn(async move {
                    let res = timeout(Duration::from_secs(15), exchanges::fetch_gateio(Some(collect_seconds))).await;
                    match res {
                        Ok(Ok(v)) => Ok(("gateio".to_string(), v)),
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

    // gather
    let mut merged_pairs: Vec<PairPrice> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for t in tasks {
        match t.await {
            Ok(Ok((_name, vec_pairs))) => {
                merged_pairs.extend(vec_pairs);
            }
            Ok(Err(e)) => errors.push(e),
            Err(e) => errors.push(format!("task join err: {}", e)),
        }
    }

    // build price map and run scan
    let price_map = logic::build_price_map(&merged_pairs);
    let results = logic::scan_triangles(&price_map, min_profit, fee_per_leg);

    // return
    (StatusCode::OK, Json(json!({
        "status": "ok",
        "errors": errors,
        "count_pairs": merged_pairs.len(),
        "count_opportunities": results.len(),
        "results": results
    })))
                }
