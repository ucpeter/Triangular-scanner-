// src/exchanges/kucoin.rs
use futures::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use tracing::{info, warn, error};
use reqwest::Client;

use crate::models::PairPrice;

pub async fn connect_kucoin(tx: tokio::sync::mpsc::Sender<PairPrice>) {
    info!("connecting to Kucoin WS…");

    // Step 1: request a token from Kucoin
    let token_url = "https://api.kucoin.com/api/v1/bullet-public";
    let client = Client::new();

    let token_resp: Value = match client.post(token_url).send().await {
        Ok(resp) => match resp.json().await {
            Ok(json) => json,
            Err(e) => {
                error!("❌ kucoin token decode error: {}", e);
                return;
            }
        },
        Err(e) => {
            error!("❌ kucoin token request error: {}", e);
            return;
        }
    };

    let token = token_resp["data"]["token"].as_str().unwrap_or_default().to_string();
    let endpoint = token_resp["data"]["instanceServers"][0]["endpoint"]
        .as_str()
        .unwrap_or("wss://ws-api-spot.kucoin.com")
        .to_string();

    if token.is_empty() {
        error!("❌ kucoin missing WS token");
        return;
    }

    let url = format!("{endpoint}?token={token}");
    let ws_url = Url::parse(&url).expect("invalid Kucoin WS url");

    match connect_async(ws_url).await {
        Ok((mut ws_stream, _)) => {
            info!("✅ connected to Kucoin WS");

            // Subscribe to all tickers
            let sub_msg = serde_json::json!({
                "id": "scanner",
                "type": "subscribe",
                "topic": "/market/ticker:all",
                "privateChannel": false,
                "response": true
            });

            if let Err(e) = ws_stream.send(Message::Text(sub_msg.to_string())).await {
                error!("❌ failed to subscribe to Kucoin: {}", e);
                return;
            }

            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(Message::Text(txt)) => {
                        if let Ok(json) = serde_json::from_str::<Value>(&txt) {
                            if json.get("type") == Some(&Value::String("pong".into())) {
                                continue;
                            }

                            if let Some(data) = json.get("data") {
                                if let (Some(symbol), Some(price_str)) =
                                    (data.get("symbol"), data.get("price"))
                                {
                                    if let (Some(symbol), Some(price_str)) =
                                        (symbol.as_str(), price_str.as_str())
                                    {
                                        if let Ok(price) = price_str.parse::<f64>() {
                                            if price > 0.0 {
                                                if let Some((base, quote)) = parse_symbol(symbol) {
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
                    Ok(Message::Ping(p)) => {
                        if let Err(e) = ws_stream.send(Message::Pong(p)).await {
                            warn!("failed to reply pong to Kucoin: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        warn!("Kucoin closed connection");
                        break;
                    }
                    Err(e) => {
                        error!("Kucoin WS error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            error!("❌ kucoin connect error: {}", e);
        }
    }
}

fn parse_symbol(sym: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = sym.split('-').collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
    }
