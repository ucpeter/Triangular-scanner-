use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tracing::{info, warn};

use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;

/// Gate.io public websocket (spot tickers) — gateway: wss://ws.gateio.ws/v4/ws
/// We'll subscribe to "spot.tickers" and parse ticker updates.
/// Note: Gate's payloads may differ; inspect logs if parsing issues appear.
pub async fn run_gateio_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://ws.gateio.ws/v4/ws";
    info!("connecting to Gate.io WS {}", url);
    let (mut ws, _) = connect_async(url).await?;

    // subscribe to all spot tickers
    let subscribe = serde_json::json!({
        "time": chrono::Utc::now().timestamp_millis(),
        "channel": "spot.tickers",
        "event": "subscribe",
        "payload": []
    });
    ws.send(tokio_tungstenite::tungstenite::Message::Text(subscribe.to_string())).await?;

    let mut local_map: HashMap<String, PairPrice> = HashMap::new();
    let (_write, mut read) = ws.split();
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if msg.is_text() {
            let text = msg.into_text()?;
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                // Gate sends {"time":..,"channel":"spot.tickers","event":"update","result":[{...}, ...]}
                if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                    if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                        for it in arr {
                            // expected fields: currency_pair or symbol, last
                            if let (Some(sym), Some(last)) = (it.get("currency_pair").and_then(|s| s.as_str()), it.get("last").and_then(|s| s.as_str())) {
                                let symbol = sym.to_uppercase();
                                if let Ok(price) = last.parse::<f64>() {
                                    let parts: Vec<&str> = symbol.split('_').collect();
                                    if parts.len() == 2 && price > 0.0 {
                                        local_map.insert(symbol.clone(), PairPrice { base: parts[0].to_string(), quote: parts[1].to_string(), price });
                                    }
                                }
                            }
                        }
                    }

                    // flush
                    let mut guard = prices.write().await;
                    guard.insert("gateio".to_string(), local_map.values().cloned().collect());
                }
            } else {
                warn!("gateio: parse failed (first 120 chars): {}", &text.chars().take(120).collect::<String>());
            }
        }
    }

    Ok(())
      }
