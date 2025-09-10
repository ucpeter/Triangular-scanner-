// src/exchanges/gateio.rs
use futures::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use tracing::{info, warn, error};

use crate::models::PairPrice;

pub async fn connect_gateio(tx: tokio::sync::mpsc::Sender<PairPrice>) {
    let url = "wss://api.gateio.ws/ws/v4/";
    let ws_url = Url::parse(url).expect("invalid Gate.io WS url");

    match connect_async(ws_url).await {
        Ok((mut ws_stream, _)) => {
            info!("✅ connected to Gate.io WS");

            // Subscribe to all spot tickers
            let sub_msg = serde_json::json!({
                "time": chrono::Utc::now().timestamp(),
                "channel": "spot.tickers",
                "event": "subscribe"
            });

            if let Err(e) = ws_stream.send(Message::Text(sub_msg.to_string())).await {
                error!("❌ failed to subscribe to Gate.io: {}", e);
                return;
            }

            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(Message::Text(txt)) => {
                        if let Ok(json) = serde_json::from_str::<Value>(&txt) {
                            if json.get("event") == Some(&Value::String("update".into())) {
                                if let Some(result) = json.get("result") {
                                    if let (Some(symbol), Some(price_str)) =
                                        (result.get("currency_pair"), result.get("last"))
                                    {
                                        if let (Some(symbol), Some(price_str)) =
                                            (symbol.as_str(), price_str.as_str())
                                        {
                                            if let Ok(price) = price_str.parse::<f64>() {
                                                if price > 0.0 {
                                                    let parts: Vec<&str> =
                                                        symbol.split('_').collect();
                                                    if parts.len() == 2 {
                                                        let _ = tx
                                                            .send(PairPrice {
                                                                base: parts[0].to_string(),
                                                                quote: parts[1].to_string(),
                                                                price,
                                                                is_spot: true,
                                                            })
                                                            .await;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Message::Ping(p)) => {
                        if let Err(e) = ws_stream.send(Message::Pong(p)).await {
                            warn!("failed to reply pong to Gate.io: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        warn!("Gate.io closed connection");
                        break;
                    }
                    Err(e) => {
                        error!("Gate.io WS error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            error!("❌ gateio connect error: {}", e);
        }
    }
                                                    }
