use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tracing::{info, warn};

use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;

/// KuCoin requires an endpoint discovery call to get the WS endpoint. The "public" endpoint typically returns an endpoint to connect.
/// For simplicity here we use the documented endpoint for broadcasting (ws://ws.kucoin.com/endpoint) — in production use the REST /api/v1/bullet-public to get a temporary endpoint.
pub async fn run_kucoin_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // NOTE: KuCoin uses a small handshake to get the real websocket endpoint with token. This example uses the static endpoint and subscribes to /market/ticker:all
    // In production you should GET https://api.kucoin.com/api/v1/bullet-public then use returned "instanceServers[0].endpoint" and token.
    let url = "wss://ws-api.kucoin.com/endpoint";
    info!("connecting to KuCoin WS {}", url);
    let (mut ws, _) = connect_async(url).await?;

    // Subscribe to the aggregated all tickers topic (requires token in real flow). Here we attempt subscription; you may need to replace endpoint with the returned one from REST.
    let subscribe = serde_json::json!({
        "id": "subscribe_ticker_all",
        "type": "subscribe",
        "topic": "/market/ticker:all",
        "response": true
    });
    ws.send(tokio_tungstenite::tungstenite::Message::Text(subscribe.to_string())).await?;

    let mut local_map: HashMap<String, PairPrice> = HashMap::new();
    let (_write, mut read) = ws.split();
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if msg.is_text() {
            let text = msg.into_text()?;
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                // KuCoin sends object with "data":{...} on topic /market/ticker:all (might be per symbol). The shape differs; adapt as needed.
                if let Some(topic) = v.get("topic").and_then(|t| t.as_str()) {
                    if topic.starts_with("/market/ticker") {
                        if let Some(data) = v.get("data") {
                            // If data is object with price fields, parse accordingly
                            if let Some(symbol) = data.get("symbol").and_then(|s| s.as_str()) {
                                let price_opt = data.get("price").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                                    .or_else(|| data.get("last").and_then(|p| p.as_f64()));
                                if let Some(price) = price_opt {
                                    let (base, quote) = split_symbol(symbol);
                                    if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                        local_map.insert(symbol.to_uppercase(), PairPrice { base, quote, price });
                                    }
                                }
                            }
                        }
                        // flush
                        let mut guard = prices.write().await;
                        guard.insert("kucoin".to_string(), local_map.values().cloned().collect());
                    }
                } else {
                    // ignore other messages
                }
            } else {
                warn!("kucoin: msg parse failed (first 120 chars): {}", &text.chars().take(120).collect::<String>());
            }
        }
    }

    Ok(())
}

fn split_symbol(sym: &str) -> (String, String) {
    // KuCoin often uses "BASE-QUOTE"
    if sym.contains('-') {
        let parts: Vec<&str> = sym.split('-').collect();
        if parts.len() == 2 {
            return (parts[0].to_string(), parts[1].to_string());
        }
    }
    (String::new(), String::new())
                      }
