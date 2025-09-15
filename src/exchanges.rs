use crate::models::PairPrice;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{info, warn, error};

/// Collects a snapshot of market prices for a given exchange over `seconds`.
pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    let url = match exchange {
        "binance" => "wss://stream.binance.com:9443/ws/!ticker@arr",
        "bybit"   => "wss://stream.bybit.com/v5/public/spot",
        "gateio"  => "wss://api.gateio.ws/ws/v4/",
        "kucoin"  => "wss://ws-api-spot.kucoin.com/?token=public",
        _ => {
            warn!("Unknown exchange {}, defaulting to Binance", exchange);
            "wss://stream.binance.com:9443/ws/!ticker@arr"
        }
    };

    info!("Connecting to {} at {}", exchange, url);
    let mut pairs: Vec<PairPrice> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(seconds);

    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            // Send subscription message where required
            if exchange == "bybit" {
                let sub = serde_json::json!({ "op": "subscribe", "args": ["tickers"] });
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
                    "response": true
                });
                let _ = ws_stream.send(Message::Text(sub.to_string())).await;
            }

            while Instant::now() < deadline {
                tokio::select! {
                    msg = ws_stream.next() => {
                        match msg {
                            Some(Ok(m)) if m.is_text() => {
                                if let Ok(txt) = m.into_text() {
                                    if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                        parse_message(exchange, &v, &mut pairs);
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                error!("{} ws error: {:?}", exchange, e);
                                break;
                            }
                            None => break,
                        }
                    }
                    _ = tokio::time::sleep_until(deadline) => break,
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

/// Dispatch parsing depending on exchange
fn parse_message(exchange: &str, v: &Value, pairs: &mut Vec<PairPrice>) {
    match exchange {
        "binance" => parse_binance(v, pairs),
        "bybit"   => parse_bybit(v, pairs),
        "gateio"  => parse_gateio(v, pairs),
        "kucoin"  => parse_kucoin(v, pairs),
        _ => {}
    }
}

fn parse_binance(v: &Value, pairs: &mut Vec<PairPrice>) {
    if let Value::Array(arr) = v {
        for it in arr {
            if let (Some(sym), Some(price)) = (
                it.get("s").and_then(|v| v.as_str()),
                it.get("c").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
            ) {
                let (base, quote) = split_symbol(sym);
                if price > 0.0 && !base.is_empty() {
                    pairs.push(PairPrice { base, quote, price, is_spot: true });
                }
            }
        }
    }
}

fn parse_bybit(v: &Value, pairs: &mut Vec<PairPrice>) {
    if v.get("topic").and_then(|t| t.as_str()) == Some("tickers") {
        if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
            for it in arr {
                if let (Some(sym), Some(price)) = (
                    it.get("symbol").and_then(|s| s.as_str()),
                    it.get("lastPrice").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                ) {
                    let (base, quote) = split_symbol(sym);
                    if price > 0.0 && !base.is_empty() {
                        pairs.push(PairPrice { base, quote, price, is_spot: true });
                    }
                }
            }
        }
    }
}

fn parse_gateio(v: &Value, pairs: &mut Vec<PairPrice>) {
    if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
        if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
            for it in arr {
                if let (Some(sym), Some(last)) = (
                    it.get("currency_pair").and_then(|s| s.as_str()),
                    it.get("last").and_then(|s| s.as_f64())
                ) {
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
}

fn parse_kucoin(v: &Value, pairs: &mut Vec<PairPrice>) {
    if let Some(data) = v.get("data") {
        if let (Some(sym), Some(price_str)) = (
            data.get("symbol").and_then(|s| s.as_str()),
            data.get("price").and_then(|p| p.as_str())
        ) {
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

/// Split Binance/Bybit-style symbols
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

/// Parse Kucoin symbols like "BTC-USDT"
fn parse_symbol(sym: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = sym.split('-').collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
                }
