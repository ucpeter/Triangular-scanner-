use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tracing::{info, warn, error};
use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};

/// NOTE: KuCoin production uses bullet-public to get a temporary WS endpoint. This simplified flow tries to connect
/// to ws://ws-api.kucoin.com/endpoint but may require the bullet flow in production.
pub async fn run_kucoin_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://ws-api.kucoin.com/endpoint";
    info!("kucoin: connecting to {}", url);

    loop {
        match connect_async(url).await {
            Ok((mut ws_stream, _)) => {
                info!("kucoin: connected, attempting subscribe");
                let sub = serde_json::json!({
                    "id": "1",
                    "type": "subscribe",
                    "topic": "/market/ticker:all",
                    "response": true
                });
                if let Err(e) = ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(sub.to_string())).await {
                    warn!("kucoin subscribe failed: {:?}", e);
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
                                            if topic.starts_with("/market/ticker") {
                                                if let Some(data) = v.get("data") {
                                                    if let Some(sym) = data.get("symbol").and_then(|s| s.as_str()) {
                                                        let price_opt = data.get("price").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                                                            .or_else(|| data.get("last").and_then(|p| p.as_f64()));
                                                        if let Some(price) = price_opt {
                                                            let (base, quote) = split_symbol(sym);
                                                            if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                                                local.insert(sym.to_uppercase(), PairPrice { base, quote, price, is_spot: true });
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
                            error!("kucoin ws read error: {:?}", e);
                            break;
                        }
                    }

                    if last_flush.elapsed() >= Duration::from_secs(1) {
                        let mut g = prices.write().await;
                        g.insert("kucoin".to_string(), local.values().cloned().collect());
                        last_flush = Instant::now();
                    }
                }

                warn!("kucoin disconnected, reconnect in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("kucoin connect error: {:?}", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

fn split_symbol(sym: &str) -> (String, String) {
    if sym.contains('-') {
        let parts: Vec<&str> = sym.split('-').collect();
        if parts.len() == 2 {
            return (parts[0].to_string(), parts[1].to_string());
        }
    }
    (String::new(), String::new())
            }
