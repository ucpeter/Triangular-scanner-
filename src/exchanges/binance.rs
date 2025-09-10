use futures::StreamExt;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tracing::{info, warn, error};

use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;

/// Binance aggregated all-tickers: wss://stream.binance.com:9443/ws/!ticker@arr
pub async fn run_binance_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("binance: connecting to {}", url);

    loop {
        match connect_async(url).await {
            Ok((ws, _)) => {
                info!("binance: connected");
                let (_write, mut read) = ws.split();
                let mut local: HashMap<String, PairPrice> = HashMap::new();
                let mut last_flush = tokio::time::Instant::now();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(m) => {
                            if m.is_text() {
                                if let Ok(txt) = m.into_text() {
                                    match serde_json::from_str::<Value>(&txt) {
                                        Ok(Value::Array(arr)) => {
                                            for item in arr {
                                                if let Some(sym) = item.get("s").and_then(|v| v.as_str()) {
                                                    let price_opt = item.get("c")
                                                        .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                                        .or_else(|| item.get("c").and_then(|v| v.as_f64()));
                                                    if let Some(price) = price_opt {
                                                        let (base, quote) = split_symbol(sym);
                                                        if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                            local.insert(sym.to_string(), PairPrice{ base, quote, price });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Ok(Value::Object(obj)) => {
                                            // envelope `data` shape
                                            if let Some(data) = obj.get("data") {
                                                if let Some(arr) = data.as_array() {
                                                    for item in arr {
                                                        if let Some(sym) = item.get("s").and_then(|v| v.as_str()) {
                                                            let price_opt = item.get("c")
                                                                .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                                                .or_else(|| item.get("c").and_then(|v| v.as_f64()));
                                                            if let Some(price) = price_opt {
                                                                let (base, quote) = split_symbol(sym);
                                                                if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                                    local.insert(sym.to_string(), PairPrice{ base, quote, price });
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                // single ticker object
                                                if let Some(sym) = obj.get("s").and_then(|v| v.as_str()) {
                                                    let price_opt = obj.get("c")
                                                        .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                                        .or_else(|| obj.get("c").and_then(|v| v.as_f64()));
                                                    if let Some(price) = price_opt {
                                                        let (base, quote) = split_symbol(sym);
                                                        if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                            local.insert(sym.to_string(), PairPrice{ base, quote, price });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("binance: json parse failed: {:?}", e);
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            // flush every 1s
                            if last_flush.elapsed() >= Duration::from_secs(1) {
                                let mut guard = prices.write().await;
                                guard.insert("binance".to_string(), local.values().cloned().collect());
                                last_flush = tokio::time::Instant::now();
                            }
                        }
                        Err(e) => {
                            error!("binance ws read error: {:?}", e);
                            break;
                        }
                    }
                } // while messages

                warn!("binance: disconnected, reconnecting after 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("binance ws connect error: {:?}", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    } // loop
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
    if s.len() > 4 {
        let (a,b) = s.split_at(s.len()-3);
        return (a.to_string(), b.to_string());
    }
    (String::new(), String::new())
                }
