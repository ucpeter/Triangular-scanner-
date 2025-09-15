use crate::models::PairPrice;
use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{info, warn, error};

/// Collects a snapshot of market prices for a given exchange over a few seconds.
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
    let mut seen_total = 0usize;
    let mut skipped = 0usize;

    match connect_async(url).await {
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
                                    "binance" => {
                                        if let Value::Array(arr) = v {
                                            for it in arr {
                                                seen_total += 1;
                                                if let (Some(sym), Some(price)) =
                                                    (it.get("s").and_then(|v| v.as_str()),
                                                     it.get("c").and_then(|v| v.as_str())
                                                                .and_then(|s| s.parse::<f64>().ok()))
                                                {
                                                    if price > 0.0 {
                                                        let (base, quote) = split_dynamic(sym);
                                                        if !base.is_empty() && !quote.is_empty() {
                                                            pairs.push(PairPrice { base, quote, price, is_spot: true });
                                                        } else { skipped += 1; }
                                                    } else { skipped += 1; }
                                                } else { skipped += 1; }
                                            }
                                        }
                                    }
                                    "bybit" => {
                                        if v.get("topic").and_then(|t| t.as_str()) == Some("tickers") {
                                            if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                                                for it in arr {
                                                    seen_total += 1;
                                                    if let (Some(sym), Some(price)) =
                                                        (it.get("symbol").and_then(|s| s.as_str()),
                                                         it.get("lastPrice").and_then(|p| p.as_str())
                                                                            .and_then(|s| s.parse::<f64>().ok()))
                                                    {
                                                        if price > 0.0 {
                                                            let (base, quote) = split_dynamic(sym);
                                                            if !base.is_empty() && !quote.is_empty() {
                                                                pairs.push(PairPrice { base, quote, price, is_spot: true });
                                                            } else { skipped += 1; }
                                                        } else { skipped += 1; }
                                                    } else { skipped += 1; }
                                                }
                                            }
                                        }
                                    }
                                    "gateio" => {
                                        if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                                            if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                                                for it in arr {
                                                    seen_total += 1;
                                                    if let (Some(sym), Some(last)) =
                                                        (it.get("currency_pair").and_then(|s| s.as_str()),
                                                         it.get("last").and_then(|s| s.as_f64()))
                                                    {
                                                        if last > 0.0 {
                                                            let parts: Vec<&str> = sym.split('_').collect();
                                                            if parts.len() == 2 {
                                                                pairs.push(PairPrice {
                                                                    base: parts[0].to_string(),
                                                                    quote: parts[1].to_string(),
                                                                    price: last,
                                                                    is_spot: true
                                                                });
                                                            } else { skipped += 1; }
                                                        } else { skipped += 1; }
                                                    } else { skipped += 1; }
                                                }
                                            }
                                        }
                                    }
                                    "kucoin" => {
                                        if let Some(data) = v.get("data") {
                                            seen_total += 1;
                                            if let (Some(sym), Some(price_str)) =
                                                (data.get("symbol").and_then(|s| s.as_str()),
                                                 data.get("price").and_then(|p| p.as_str()))
                                            {
                                                if let Ok(price) = price_str.parse::<f64>() {
                                                    if price > 0.0 {
                                                        let parts: Vec<&str> = sym.split('-').collect();
                                                        if parts.len() == 2 {
                                                            pairs.push(PairPrice {
                                                                base: parts[0].to_string(),
                                                                quote: parts[1].to_string(),
                                                                price,
                                                                is_spot: true
                                                            });
                                                        } else { skipped += 1; }
                                                    } else { skipped += 1; }
                                                } else { skipped += 1; }
                                            } else { skipped += 1; }
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

    info!("{} collected {} unique pairs, seen_total={}, skipped={}",
        exchange, pairs.len(), seen_total, skipped);

    pairs
}

/// Dynamic split without hardcoded suffixes.
/// Rule:
/// - If contains "-" or "_", split by it.
/// - Otherwise, take last 3 chars as quote, rest as base (fallback).
fn split_dynamic(sym: &str) -> (String, String) {
    if sym.contains('-') {
        let parts: Vec<&str> = sym.split('-').collect();
        if parts.len() == 2 {
            return (parts[0].to_string(), parts[1].to_string());
        }
    } else if sym.contains('_') {
        let parts: Vec<&str> = sym.split('_').collect();
        if parts.len() == 2 {
            return (parts[0].to_string(), parts[1].to_string());
        }
    } else if sym.len() > 3 {
        let (base, quote) = sym.split_at(sym.len() - 3);
        return (base.to_string(), quote.to_string());
    }
    (String::new(), String::new())
    }
