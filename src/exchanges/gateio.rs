use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use url::Url;
use tracing::{info, warn, error};
use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use chrono::Utc;

pub async fn run_gateio_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://api.gateio.ws/ws/v4/";
    info!("gateio: connecting to {}", url);

    loop {
        match connect_async(url).await {
            Ok((mut ws_stream, _)) => {
                info!("gateio: connected");

                // subscribe to spot.tickers
                let sub_msg = serde_json::json!({
                    "time": Utc::now().timestamp_millis(),
                    "channel":"spot.tickers",
                    "event":"subscribe",
                    "payload":[]
                });

                if let Err(e) = ws_stream.send(Message::Text(sub_msg.to_string())).await {
                    warn!("gateio subscribe failed: {:?}", e);
                }

                let (mut write, mut read) = ws_stream.split();
                let mut local: HashMap<String, PairPrice> = HashMap::new();
                let mut last_flush = Instant::now();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(m) => {
                            if m.is_text() {
                                if let Ok(txt) = m.into_text() {
                                    if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                        if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                                            if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                                                for it in arr {
                                                    if let (Some(sym), Some(last)) = (it.get("currency_pair").and_then(|s| s.as_str()), it.get("last").and_then(|s| s.as_f64())) {
                                                        let parts: Vec<&str> = sym.split('_').collect();
                                                        if parts.len() == 2 {
                                                            let base = parts[0].to_string();
                                                            let quote = parts[1].to_string();
                                                            let price = last;
                                                            if price > 0.0 {
                                                                local.insert(sym.to_uppercase(), PairPrice { base, quote, price, is_spot: true });
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else if m.is_ping() {
                                if let Err(e) = write.send(Message::Pong(vec![])).await {
                                    warn!("gateio write pong failed: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("gateio ws read error: {:?}", e);
                            break;
                        }
                    }

                    if last_flush.elapsed() >= Duration::from_secs(1) {
                        let mut guard = prices.write().await;
                        guard.insert("gateio".to_string(), local.values().cloned().collect());
                        last_flush = Instant::now();
                    }
                }

                warn!("gateio disconnected, reconnect in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("gateio connect error: {:?}", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
                                                                }
