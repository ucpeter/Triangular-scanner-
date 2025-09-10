use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use url::Url;
use tracing::{info, warn, error};
use reqwest::Client;
use crate::models::PairPrice;
use crate::ws_manager::SharedPrices;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};

pub async fn run_kucoin_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("kucoin: requesting bullet-public token");
    let client = Client::new();
    let token_resp: Value = client.post("https://api.kucoin.com/api/v1/bullet-public")
        .send().await
        .map_err(|e| format!("kucoin token request err: {}", e))?
        .json().await
        .map_err(|e| format!("kucoin token decode err: {}", e))?;

    let token = token_resp["data"]["token"].as_str().unwrap_or_default().to_string();
    let endpoint = token_resp["data"]["instanceServers"][0]["endpoint"]
        .as_str().unwrap_or("wss://ws-api-spot.kucoin.com").to_string();

    if token.is_empty() {
        error!("kucoin missing token");
        return Err("kucoin missing token".into());
    }

    let url = format!("{}?token={}", endpoint, token);
    info!("kucoin: connecting to {}", url);

    let ws_url = Url::parse(&url)?;
    loop {
        match connect_async(ws_url.clone()).await {
            Ok((mut ws_stream, _)) => {
                info!("kucoin: connected");

                // subscribe to all tickers
                let sub = serde_json::json!({
                    "id":"scanner",
                    "type":"subscribe",
                    "topic":"/market/ticker:all",
                    "response": true
                });
                if let Err(e) = ws_stream.send(Message::Text(sub.to_string())).await {
                    warn!("kucoin subscribe failed: {:?}", e);
                }

                let (mut write, mut read) = ws_stream.split();
                let mut local: HashMap<String, PairPrice> = HashMap::new();
                let mut last_flush = Instant::now();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(m) => {
                            if m.is_text() {
                                if let Ok(txt) = m.into_text() {
                                    if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                        // Kucoin: data often inside "data" -> "symbol","price"
                                        if let Some(data) = v.get("data") {
                                            if let (Some(sym), Some(price_v)) = (data.get("symbol"), data.get("price")) {
                                                if let (Some(sym), Some(price_str)) = (sym.as_str(), price_v.as_str()) {
                                                    if let Ok(price) = price_str.parse::<f64>() {
                                                        if price > 0.0 {
                                                            if let Some((base, quote)) = parse_symbol(sym) {
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
                                    warn!("kucoin write pong failed: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("kucoin ws read error: {:?}", e);
                            break;
                        }
                    }

                    if last_flush.elapsed() >= Duration::from_secs(1) {
                        let mut guard = prices.write().await;
                        guard.insert("kucoin".to_string(), local.values().cloned().collect());
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

fn parse_symbol(sym: &str) -> Option<(String,String)> {
    let parts: Vec<&str> = sym.split('-').collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
                    }
