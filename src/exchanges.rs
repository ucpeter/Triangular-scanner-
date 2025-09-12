use crate::models::PairPrice;
use tokio::time::{Duration, Instant};
use tracing::{info, warn, error};
use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use serde_json::Value;
use reqwest::Client;

/// Collects a snapshot of market prices for a given exchange over a few seconds.
pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    let url = match exchange {
        "binance" => "wss://stream.binance.com:9443/ws/!ticker@arr",
        "bybit"   => "wss://stream.bybit.com/v5/public/spot",
        "gateio"  => "wss://api.gateio.ws/ws/v4/",
        "kucoin"  => {
            // For Kucoin we must fetch a token dynamically
            if let Ok(u) = get_kucoin_ws_url().await {
                u
            } else {
                warn!("kucoin token fetch failed");
                return vec![];
            }
        }
        _ => {
            warn!("Unknown exchange {}, defaulting to Binance", exchange);
            "wss://stream.binance.com:9443/ws/!ticker@arr".to_string()
        }
    };

    info!("Connecting to {} at {}", exchange, url);
    let mut pairs: Vec<PairPrice> = Vec::new();

    match connect_async(&url).await {
        Ok((mut ws_stream, _)) => {
            // Send subscription messages if required
            if exchange == "bybit" {
                let sub = serde_json::json!({
                    "op": "subscribe",
                    "args": ["tickers"]
                });
                let _ = ws_stream.send(Message::Text(sub.to_string())).await;
            } else if exchange == "gateio" {
                let sub = serde_json::json!({
                    "time": chrono::Utc::now().timestamp_millis(),
                    "channel":"spot.tickers",
                    "event":"subscribe",
                    "payload":[]
                });
                let _ = ws_stream.send(Message::Text(sub.to_string())).await;
            } else if exchange == "kucoin" {
                let sub = serde_json::json!({
                    "id":"scanner",
                    "type":"subscribe",
                    "topic":"/market/ticker:all",
                    "privateChannel": false,
                    "response": true
                });
                let _ = ws_stream.send(Message::Text(sub.to_string())).await;
            }

            let deadline = Instant::now() + Duration::from_secs(seconds);

            while let Some(msg) = ws_stream.next().await {
                if Instant::now() >= deadline {
                    break;
                }

                match msg {
                    Ok(m) if m.is_text() => {
                        if let Ok(txt) = m.into_text() {
                            if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                match exchange {
                                    // ✅ Binance: array of tickers
                                    "binance" => {
                                        if let Value::Array(arr) = v {
                                            for it in arr {
                                                if let (Some(sym), Some(price)) =
                                                    (it.get("s").and_then(|v| v.as_str()),
                                                     it.get("c").and_then(|v| v.as_str())
                                                                .and_then(|s| s.parse::<f64>().ok()))
                                                {
                                                    let (base, quote) = split_symbol(sym);
                                                    if price > 0.0 && !base.is_empty() {
                                                        pairs.push(PairPrice { base, quote, price, is_spot: true });
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // ✅ Bybit: inside topic "tickers"
                                    "bybit" => {
                                        if v.get("topic").and_then(|t| t.as_str()) == Some("tickers") {
                                            if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                                                for it in arr {
                                                    if let (Some(sym), Some(price)) =
                                                        (it.get("symbol").and_then(|s| s.as_str()),
                                                         it.get("lastPrice").and_then(|p| p.as_str())
                                                                            .and_then(|s| s.parse::<f64>().ok()))
                                                    {
                                                        let (base, quote) = split_symbol(sym);
                                                        if price > 0.0 && !base.is_empty() {
                                                            pairs.push(PairPrice { base, quote, price, is_spot: true });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // ✅ Gate.io: result is one object, not array
                                    "gateio" => {
                                        if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                                            if let Some(result) = v.get("result") {
                                                if let (Some(sym), Some(last)) =
                                                    (result.get("currency_pair").and_then(|s| s.as_str()),
                                                     result.get("last").and_then(|s| s.as_str())
                                                             .and_then(|s| s.parse::<f64>().ok()))
                                                {
                                                    let parts: Vec<&str> = sym.split('_').collect();
                                                    if parts.len() == 2 && last > 0.0 {
                                                        pairs.push(PairPrice {
                                                            base: parts[0].to_string(),
                                                            quote: parts[1].to_string(),
                                                            price: last,
                                                            is_spot: true
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // ✅ Kucoin: under "data"
                                    "kucoin" => {
                                        if let Some(data) = v.get("data") {
                                            if let (Some(sym), Some(price_str)) =
                                                (data.get("symbol").and_then(|s| s.as_str()),
                                                 data.get("price").and_then(|p| p.as_str()))
                                            {
                                                if let Ok(price) = price_str.parse::<f64>() {
                                                    if price > 0.0 {
                                                        if let Some((base, quote)) = parse_symbol(sym) {
                                                            pairs.push(PairPrice { base, quote, price, is_spot: true });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("{} ws error: {:?}", exchange, e);
                        break;
                    }
                }
            }
        }
        Err(e) => {
            error!("{} connect error: {:?}", exchange, e);
        }
    }

    info!("{} collected {} pairs", exchange, pairs.len());
    pairs
}

/// Kucoin requires token request
async fn get_kucoin_ws_url() -> Result<String, reqwest::Error> {
    let client = Client::new();
    let resp: Value = client.post("https://api.kucoin.com/api/v1/bullet-public")
        .send().await?
        .json().await?;

    if let Some(token) = resp["data"]["token"].as_str() {
        if let Some(endpoint) = resp["data"]["instanceServers"][0]["endpoint"].as_str() {
            return Ok(format!("{}?token={}", endpoint, token));
        }
    }
    Err(reqwest::Error::new(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "missing kucoin token"))
}

/// Binance-style symbol splitting
fn split_symbol(sym: &str) -> (String, String) {
    let suffixes = ["USDT", "BUSD", "USDC", "BTC", "ETH"];
    let s = sym.to_uppercase();
    for suf in &suffixes {
        if s.ends_with(suf) && s.len() > suf.len() {
            let base = s[..s.len() - suf.len()].to_string();
            return (base, suf.to_string());
        }
    }
    (String::new(), String::new())
}

/// Kucoin symbols are like BTC-USDT
fn parse_symbol(sym: &str) -> Option<(String,String)> {
    let parts: Vec<&str> = sym.split('-').collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
        }
