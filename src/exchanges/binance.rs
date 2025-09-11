use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use std::collections::HashMap;
use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;
use tracing::{info, warn, error};
use tokio::time::{Duration, Instant};

pub async fn run_binance_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("binance: connecting to {}", url);

    let mut backoff = 2u64;
    let max_backoff = 60u64;

    loop {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                info!("binance: connected");
                let (mut write, mut read) = ws_stream.split();
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
                                            // Binance returns an array of tickers for !ticker@arr
                                            if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&txt) {
                                                for item in arr {
                                                    let sym = item.get("s").and_then(|v| v.as_str()).unwrap_or("").to_uppercase();
                                                    let price_opt = item.get("c")
                                                        .and_then(|v| v.as_str())
                                                        .and_then(|s| s.parse::<f64>().ok())
                                                        .or_else(|| item.get("c").and_then(|v| v.as_f64()));
                                                    if let Some(price) = price_opt {
                                                        let (base, quote) = split_symbol(&sym);
                                                        if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                            local.insert(sym.clone(), PairPrice { base, quote, price, is_spot: true });
                                                        }
                                                    }
                                                }
                                            } else {
                                                // sometimes single object; try parse
                                                if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&txt) {
                                                    if let Some(symv) = map.get("s") {
                                                        if let Some(sym) = symv.as_str() {
                                                            let price_opt = map.get("c")
                                                                .and_then(|v| v.as_str())
                                                                .and_then(|s| s.parse::<f64>().ok())
                                                                .or_else(|| map.get("c").and_then(|v| v.as_f64()));
                                                            if let Some(price) = price_opt {
                                                                let (base, quote) = split_symbol(&sym.to_uppercase());
                                                                if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                                    local.insert(sym.to_uppercase(), PairPrice { base, quote, price, is_spot: true });
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else if m.is_ping() {
                                        // respond with Pong
                                        if let Err(e) = write.send(Message::Pong(vec![])).await {
                                            warn!("binance write pong failed: {:?}", e);
                                        }
                                    } else if m.is_close() {
                                        warn!("binance: remote closed");
                                        break;
                                    }
                                }
                                Some(Err(e)) => {
                                    error!("binance ws read error: {:?}", e);
                                    break;
                                }
                                None => {
                                    warn!("binance read stream ended");
                                    break;
                                }
                            }

                            if last_flush.elapsed() >= Duration::from_secs(1) {
                                let mut guard = prices.write().await;
                                guard.insert("binance".to_string(), local.values().cloned().collect());
                                last_flush = Instant::now();
                            }
                        }

                        _ = ping_interval.tick() => {
                            // send client-side ping to keep connection alive
                            if let Err(e) = write.send(Message::Ping(vec![])).await {
                                warn!("binance ping send failed: {:?}", e);
                            }
                        }
                    } // select
                } // inner loop

                backoff = 2; // reset backoff on successful connect
                warn!("binance disconnected, reconnecting in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("binance connect error: {:?}", e);
                let wait = backoff.min(max_backoff);
                tokio::time::sleep(Duration::from_secs(wait)).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

fn split_symbol(sym: &str) -> (String, String) {
    let suffixes = ["USDT","BUSD","USDC","BTC","ETH","BNB"];
    let s = sym.to_uppercase();
    for suf in &suffixes {
        if s.ends_with(suf) && s.len() > suf.len() {
            let base = s[..s.len()-suf.len()].to_string();
            return (base, suf.to_string());
        }
    }
    if s.len() > 3 {
        let (a,b) = s.split_at(s.len()-3);
        return (a.to_string(), b.to_string());
    }
    (String::new(), String::new())
                                        }
