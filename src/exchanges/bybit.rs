// src/exchanges/bybit.rs
use futures::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use tracing::{info, warn, error};

use crate::models::PairPrice;

pub async fn connect_bybit(tx: tokio::sync::mpsc::Sender<PairPrice>) {
    let url = "wss://stream.bybit.com/v5/public/spot";
    info!("connecting to Bybit WS: {}", url);

    let ws_url = Url::parse(url).expect("invalid Bybit WS url");

    match connect_async(ws_url).await {
        Ok((mut ws_stream, _)) => {
            info!("✅ connected to Bybit");

            // Subscribe to tickers stream for ALL pairs
            // You can dynamically build the args, for now subscribe to "tickers" (all)
            let sub_msg = serde_json::json!({
                "op": "subscribe",
                "args": ["tickers"]
            });

            if let Err(e) = ws_stream.send(Message::Text(sub_msg.to_string())).await {
                error!("❌ failed to send subscription to Bybit: {}", e);
                return;
            }

            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(Message::Text(txt)) => {
                        if let Ok(json) = serde_json::from_str::<Value>(&txt) {
                            // Handle pings
                            if json.get("op") == Some(&Value::String("ping".to_string())) {
                                let pong = serde_json::json!({ "op": "pong" });
                                if let Err(e) = ws_stream.send(Message::Text(pong.to_string())).await {
                                    warn!("failed to reply pong to Bybit: {}", e);
                                }
                                continue;
                            }

                            // Parse tickers
                            if let Some(obj) = json.get("data") {
                                if let (Some(symbol), Some(price_str)) =
                                    (obj.get("symbol"), obj.get("lastPrice"))
                                {
                                    if let (Some(symbol), Some(price_str)) =
                                        (symbol.as_str(), price_str.as_str())
                                    {
                                        if let Ok(price) = price_str.parse::<f64>() {
                                            if price > 0.0 {
                                                let (base, quote) = split_symbol(symbol);
                                                if !base.is_empty() && !quote.is_empty() {
                                                    let _ = tx.send(PairPrice {
                                                        base,
                                                        quote,
                                                        price,
                                                        is_spot: true,
                                                    }).await;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Message::Ping(_)) => {
                        // Reply tungstenite-level ping
                        if let Err(e) = ws_stream.send(Message::Pong(vec![])).await {
                            warn!("failed to reply tungstenite pong: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        warn!("Bybit closed connection");
                        break;
                    }
                    Err(e) => {
                        error!("Bybit WS error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            error!("❌ bybit connect error: {}", e);
        }
    }
}

/// Split Bybit symbols into base/quote
fn split_symbol(symbol: &str) -> (String, String) {
    let suffixes = ["USDT", "USDC", "BTC", "ETH"];
    for s in suffixes {
        if symbol.ends_with(s) && symbol.len() > s.len() {
            let base = symbol.trim_end_matches(s).to_string();
            return (base, s.to_string());
        }
    }
    (String::new(), String::new())
            }
