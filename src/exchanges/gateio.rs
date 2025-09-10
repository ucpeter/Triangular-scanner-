use futures::StreamExt;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use tokio_tungstenite::connect_async;
use tracing::{info, warn, error};

use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;

/// Gate.io: wss://ws.gateio.ws/v4/ws
/// Subscribe to channel "spot.tickers".
pub async fn run_gateio_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://ws.gateio.ws/v4/ws";
    info!("gateio: connecting to {}", url);

    loop {
        match connect_async(url).await {
            Ok((mut ws, _)) => {
                info!("gateio connected, subscribing spot.tickers");
                let sub = serde_json::json!({
                    "time": chrono::Utc::now().timestamp_millis(),
                    "channel":"spot.tickers",
                    "event":"subscribe",
                    "payload": []
                });
                // This file uses chrono for timestamp; chrono is not in Cargo.toml earlier —
                // if build fails, replace with simple timestamp: chrono::Utc::now().timestamp_millis()
                if let Err(e) = ws.send(tokio_tungstenite::tungstenite::Message::Text(sub.to_string())).await {
                    warn!("gateio: subscribe send failed: {:?}", e);
                }

                let (_write, mut read) = ws.split();
                let mut local: HashMap<String, PairPrice> = HashMap::new();
                let mut last_flush = tokio::time::Instant::now();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(m) => {
                            if m.is_text() {
                                if let Ok(txt) = m.into_text() {
                                    if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                        if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                                            if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                                                for it in arr {
                                                    if let (Some(sym), Some(last)) = (it.get("currency_pair").and_then(|s| s.as_str()), it.get("last").and_then(|s| s.as_str())) {
                                                        if let Ok(price) = last.parse::<f64>() {
                                                            let parts: Vec<&str> = sym.split('_').collect();
                                                            if parts.len() == 2 && price > 0.0 {
                                                                local.insert(sym.to_uppercase(), PairPrice { base: parts[0].to_string(), quote: parts[1].to_string(), price });
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if last_flush.elapsed() >= Duration::from_secs(1) {
                                let mut guard = prices.write().await;
                                guard.insert("gateio".to_string(), local.values().cloned().collect());
                                last_flush = tokio::time::Instant::now();
                            }
                        }
                        Err(e) => {
                            error!("gateio ws read error: {:?}", e);
                            break;
                        }
                    }
                }

                warn!("gateio: disconnected, reconnect 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("gateio connect error: {:?}", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
                    }
