use futures_util::{StreamExt};
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

    loop {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                info!("binance: connected");
                let (_write, mut read) = ws_stream.split();
                let mut local: HashMap<String, PairPrice> = HashMap::new();
                let mut last_flush = Instant::now();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(m) => {
                            if m.is_text() {
                                if let Ok(txt) = m.into_text() {
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
                                    }
                                }
                            } else if m.is_ping() {
                                // ignore; tungstenite handles ping/pong at lower level
                            }
                        }
                        Err(e) => {
                            error!("binance ws read error: {:?}", e);
                            break;
                        }
                    }

                    if last_flush.elapsed() >= Duration::from_secs(1) {
                        let mut guard = prices.write().await;
                        guard.insert("binance".to_string(), local.values().cloned().collect());
                        last_flush = Instant::now();
                    }
                }

                warn!("binance disconnected, reconnecting in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("binance connect error: {:?}", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
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
    // fallback: last 3 chars as quote
    if s.len() > 3 {
        let (a,b) = s.split_at(s.len()-3);
        return (a.to_string(), b.to_string());
    }
    (String::new(), String::new())
                                    }
