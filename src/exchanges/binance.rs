// src/exchanges/binance.rs
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;

/// Binance aggregated "pro" all-tickers stream:
/// wss://stream.binance.com:9443/ws/!ticker@arr
///
/// This worker maintains a local HashMap<symbol, PairPrice>, and periodically
/// writes a Vec<PairPrice> into the shared `prices` under key "binance".
pub async fn run_binance_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("binance: connecting to {}", url);

    // reconnect loop
    loop {
        match connect_async(url).await {
            Ok((ws_stream, _resp)) => {
                info!("binance: connected, starting listener");
                let (_write, mut read) = ws_stream.split();

                // local map: symbol -> PairPrice
                let mut local_map: HashMap<String, PairPrice> = HashMap::new();
                // We'll flush to shared map periodically (every X messages / seconds).
                let mut last_flush = tokio::time::Instant::now();

                while let Some(msg_res) = read.next().await {
                    match msg_res {
                        Ok(msg) => {
                            if msg.is_text() {
                                let text = match msg.into_text() {
                                    Ok(t) => t,
                                    Err(e) => {
                                        warn!("binance: msg.into_text error: {:?}", e);
                                        continue;
                                    }
                                };

                                // Binance sometimes encloses arrays directly (each message may be an array)
                                // Try parse as JSON value
                                match serde_json::from_str::<Value>(&text) {
                                    Ok(Value::Array(arr)) => {
                                        // array of tickers
                                        for item in arr {
                                            if let Some(symbol) = item.get("s").and_then(|v| v.as_str()) {
                                                // Price can be "c": "123.45" or as number
                                                let price_opt = item.get("c")
                                                    .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                                    .or_else(|| item.get("c").and_then(|v| v.as_f64()));

                                                if let Some(price) = price_opt {
                                                    if let (base, quote) = split_symbol_binance(symbol) {
                                                        if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                            let pp = PairPrice {
                                                                base,
                                                                quote,
                                                                price,
                                                            };
                                                            local_map.insert(symbol.to_string(), pp);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Ok(Value::Object(obj)) => {
                                        // Sometimes Binance sends object with "data" or other envelope
                                        // Try common shapes: { "stream": "...", "data": {...} } or single ticker object
                                        if let Some(data) = obj.get("data") {
                                            if let Some(arr) = data.as_array() {
                                                for item in arr {
                                                    if let Some(symbol) = item.get("s").and_then(|v| v.as_str()) {
                                                        let price_opt = item.get("c")
                                                            .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                                            .or_else(|| item.get("c").and_then(|v| v.as_f64()));
                                                        if let Some(price) = price_opt {
                                                            if let (base, quote) = split_symbol_binance(symbol) {
                                                                if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                                    let pp = PairPrice {
                                                                        base,
                                                                        quote,
                                                                        price,
                                                                    };
                                                                    local_map.insert(symbol.to_string(), pp);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            } else if let Some(item) = data.as_object() {
                                                if let Some(symbol) = item.get("s").and_then(|v| v.as_str()) {
                                                    let price_opt = item.get("c")
                                                        .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                                        .or_else(|| item.get("c").and_then(|v| v.as_f64()));
                                                    if let Some(price) = price_opt {
                                                        if let (base, quote) = split_symbol_binance(symbol) {
                                                            if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                                let pp = PairPrice {
                                                                    base,
                                                                    quote,
                                                                    price,
                                                                };
                                                                local_map.insert(symbol.to_string(), pp);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            // possibly a single ticker object
                                            if let Some(symbol) = obj.get("s").and_then(|v| v.as_str()) {
                                                let price_opt = obj.get("c")
                                                    .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                                    .or_else(|| obj.get("c").and_then(|v| v.as_f64()));
                                                if let Some(price) = price_opt {
                                                    if let (base, quote) = split_symbol_binance(symbol) {
                                                        if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                            let pp = PairPrice {
                                                                base,
                                                                quote,
                                                                price,
                                                            };
                                                            local_map.insert(symbol.to_string(), pp);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // occasionally messages not json or compressed; log and continue
                                        warn!("binance: json parse failed: {:?} (first 120 chars: {})", e, &text.chars().take(120).collect::<String>());
                                    }
                                    _ => {}
                                }

                                // Flush to shared prices every 1 second (or when many updates)
                                if last_flush.elapsed() >= Duration::from_secs(1) {
                                    let mut guard = prices.write().await;
                                    let vec: Vec<PairPrice> = local_map.values().cloned().collect();
                                    guard.insert("binance".to_string(), vec);
                                    last_flush = tokio::time::Instant::now();
                                }
                            } else if msg.is_ping() || msg.is_pong() {
                                // ignore
                            } else {
                                // ignore binary or other messages
                            }
                        }
                        Err(e) => {
                            warn!("binance: ws message error: {:?}", e);
                            break; // break to reconnect
                        }
                    } // match msg_res
                } // while let Some(msg)

                warn!("binance: websocket stream ended — will reconnect in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue; // outer loop will reconnect
            } // Ok(ws_stream)
            Err(e) => {
                error!("binance: connect error: {:?} — retrying in 3s", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        } // match connect_async
    } // loop
}

/// Split Binance symbols robustly into (base, quote)
fn split_symbol_binance(sym: &str) -> (String, String) {
    // common quote suffixes on Binance (order matters: longer first)
    let suffixes = ["BUSD", "USDT", "USDC", "BTC", "ETH", "BNB"];
    for s in &suffixes {
        if sym.ends_with(s) && sym.len() > s.len() {
            let base = sym[..sym.len() - s.len()].to_string();
            return (base, s.to_string());
        }
    }
    // fallback: split into letters/digits boundary or last 3/4 chars
    if sym.len() > 4 {
        // try last 3
        let (a, b) = sym.split_at(sym.len() - 3);
        return (a.to_string(), b.to_string());
    }
    (String::new(), String::new())
                                    }
