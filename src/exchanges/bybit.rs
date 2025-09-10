use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;
use tracing::{info, warn, error};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};

pub async fn run_bybit_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://stream.bybit.com/v5/public/spot";
    info!("bybit: connecting to {}", url);

    loop {
        match connect_async(url).await {
            Ok((mut ws_stream, _)) => {
                info!("bybit: connected");
                // subscribe to tickers
                let sub = serde_json::json!({
                    "op": "subscribe",
                    "args": ["tickers"]
                });
                if let Err(e) = ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(sub.to_string())).await {
                    warn!("bybit subscribe send failed: {:?}", e);
                }

                let (_write, mut read) = ws_stream.split();
                let mut local: HashMap<String, PairPrice> = HashMap::new();
                let mut last_flush = Instant::now();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(m) => {
                            if m.is_text() {
                                if let Ok(txt) = m.into_text() {
                                    if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                        if let Some(topic) = v.get("topic").and_then(|t| t.as_str()) {
                                            if topic == "tickers" {
                                                if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                                                    for it in arr {
                                                        let sym = it.get("symbol").and_then(|s| s.as_str()).unwrap_or("").to_uppercase();
                                                        let price_opt = it.get("lastPrice").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                                                            .or_else(|| it.get("lastPrice").and_then(|p| p.as_f64()));
                                                        if let Some(price) = price_opt {
                                                            let (base, quote) = split_symbol(&sym);
                                                            if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                                local.insert(sym.clone(), PairPrice { base, quote, price, is_spot: true });
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("bybit ws read error: {:?}", e);
                            break;
                        }
                    }

                    if last_flush.elapsed() >= Duration::from_secs(1) {
                        let mut g = prices.write().await;
                        g.insert("bybit".to_string(), local.values().cloned().collect());
                        last_flush = Instant::now();
                    }
                }

                warn!("bybit disconnected, reconnect in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("bybit connect error: {:?}", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

fn split_symbol(sym: &str) -> (String, String) {
    let suffixes = ["USDT","USDC","BTC","ETH"];
    for suf in &suffixes {
        if sym.ends_with(suf) && sym.len() > suf.len() {
            let base = sym[..sym.len()-suf.len()].to_string();
            return (base, suf.to_string());
        }
    }
    (String::new(), String::new())
                                            }
