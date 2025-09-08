use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tracing::{info, warn};

use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;

/// Bybit aggregated tickers: wss://stream.bybit.com/v5/public/spot
/// We subscribe to "tickers" to receive continuous updates for spot market.
/// Note: Bybit messages sometimes come with {"topic":"tickers", "data":[ ... ] }
pub async fn run_bybit_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://stream.bybit.com/v5/public/spot";
    info!("connecting to Bybit WS {}", url);
    let (mut ws, _) = connect_async(url).await?;
    // subscribe to all tickers
    let subscribe = serde_json::json!({
        "op": "subscribe",
        "args": ["tickers"]
    });
    ws.send(tokio_tungstenite::tungstenite::Message::Text(subscribe.to_string())).await?;

    let mut local_map: HashMap<String, PairPrice> = HashMap::new();

    let (_ws_write, mut read) = ws.split();
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if msg.is_text() {
            let text = msg.into_text()?;
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                // Typical shape: {"topic":"tickers","data":[{"symbol":"BTCUSDT","lastPrice":"30000", ...}, ...], "ts":...}
                if let Some(topic) = v.get("topic").and_then(|t| t.as_str()) {
                    if topic == "tickers" {
                        if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                            for it in arr {
                                let sym = it.get("symbol").and_then(|s| s.as_str()).unwrap_or("").to_uppercase();
                                let price_opt = it.get("lastPrice").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                                    .or_else(|| it.get("lastPrice").and_then(|p| p.as_f64()));
                                if let Some(price) = price_opt {
                                    // split by known suffixes (same logic as Binance)
                                    let (base, quote) = split_symbol(sym.as_str());
                                    if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                        local_map.insert(sym.clone(), PairPrice { base, quote, price });
                                    }
                                }
                            }

                            // flush to shared
                            let mut guard = prices.write().await;
                            guard.insert("bybit".to_string(), local_map.values().cloned().collect());
                        }
                    }
                } else {
                    // Bybit could also send heartbeat/other messages
                }
            } else {
                warn!("bybit: failed to parse ws message as json (first 80 chars): {}", &text.chars().take(80).collect::<String>());
            }
        }
    }

    Ok(())
}

fn split_symbol(sym: &str) -> (String, String) {
    let suffixes = ["USDT", "USDC", "BTC", "ETH"];
    for s in &suffixes {
        if sym.ends_with(s) && sym.len() > s.len() {
            let base = sym.trim_end_matches(s).to_string();
            return (base, s.to_string());
        }
    }
    (String::new(), String::new())
      }
