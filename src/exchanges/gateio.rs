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

    let mut backoff = 2u64;
    let max_backoff = 60u64;

    loop {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                info!("gateio: connected");
                let (mut write, mut read) = ws_stream.split();

                // subscribe to spot.tickers
                let sub_msg = serde_json::json!({
                    "time": Utc::now().timestamp_millis(),
                    "channel":"spot.tickers",
                    "event":"subscribe",
                    "payload":[]
                });

                if let Err(e) = write.send(Message::Text(sub_msg.to_string())).await {
                    warn!("gateio subscribe failed: {:?}", e);
                }

                let mut local: HashMap<String, PairPrice> = HashMap::new();
                let mut last_flush = Instant::now();
                let mut ping_interval = tokio::time::interval(Duration::from_secs(20));
                ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                loop {
                    tokio::select! {
                        msg = read.next() => {
                            match msg {
                                Some(Ok(m)) => {
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
                                    } else if m.is_close() {
                                        warn!("gateio: remote closed");
                                        break;
                                    }
                                }
                                Some(Err(e)) => {
                                    error!("gateio ws read error: {:?}", e);
                                    break;
                                }
                                None => {
                                    warn!("gateio read ended");
                                    break;
                                }
                            }

                            if last_flush.elapsed() >= Duration::from_secs(1) {
                                let mut guard = prices.write().await;
                                guard.insert("gateio".to_string(), local.values().cloned().collect());
                                last_flush = Instant::now();
                            }
                        }

                        _ = ping_interval.tick() => {
                            // send keepalive ping
                            if let Err(e) = write.send(Message::Ping(vec![])).await {
                                warn!("gateio ping failed: {:?}", e);
                            }
                        }
                    } // select
                } // inner loop

                backoff = 2;
                warn!("gateio disconnected, reconnecting in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("gateio connect error: {:?}", e);
                let wait = backoff.min(max_backoff);
                tokio::time::sleep(Duration::from_secs(wait)).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
                    }
