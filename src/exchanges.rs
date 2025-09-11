// src/exchanges.rs
use crate::models::PairPrice;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{error, info, warn};
use url::Url;
use reqwest::Client;

/// default snapshot window (seconds)
const DEFAULT_COLLECT_SECONDS: u64 = 2;

/// best-effort symbol splitter for common formats
fn split_symbol_generic(sym: &str) -> Option<(String, String)> {
    let s = sym.to_uppercase();
    let candidates = ["USDT","BUSD","USDC","BTC","ETH","BNB","ADA","DOT","SOL"];
    for q in &candidates {
        if s.ends_with(q) && s.len() > q.len() {
            let base = s[..s.len() - q.len()].to_string();
            return Some((base, q.to_string()));
        }
    }
    if sym.contains('/') {
        let p: Vec<&str> = sym.split('/').collect();
        if p.len() == 2 { return Some((p[0].to_string(), p[1].to_string())); }
    }
    if sym.contains('-') {
        let p: Vec<&str> = sym.split('-').collect();
        if p.len() == 2 { return Some((p[0].to_string(), p[1].to_string())); }
    }
    if sym.contains('_') {
        let p: Vec<&str> = sym.split('_').collect();
        if p.len() == 2 { return Some((p[0].to_string(), p[1].to_string())); }
    }
    None
}

/// fetch binance tickers via WS one-shot
pub async fn fetch_binance(collect_seconds: Option<u64>) -> Result<Vec<PairPrice>, String> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("binance: connecting {}", url);
    let ws_url = Url::parse(url).map_err(|e| format!("binance url parse: {}", e))?;
    let timeout = Duration::from_secs(12);

    let (ws, _) = tokio::time::timeout(timeout, connect_async(ws_url))
        .await
        .map_err(|_| "binance connect timeout".to_string())?
        .map_err(|e| format!("binance connect error: {}", e))?;

    let (mut write, mut read) = ws.split();
    let collect = collect_seconds.unwrap_or(DEFAULT_COLLECT_SECONDS);
    let end = Instant::now() + Duration::from_secs(collect);
    let mut seen: HashMap<String, PairPrice> = HashMap::new();

    while Instant::now() < end {
        match tokio::time::timeout(Duration::from_secs(collect), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if msg.is_text() {
                    if let Ok(txt) = msg.into_text() {
                        if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&txt) {
                            for item in arr {
                                let symbol = item.get("s").and_then(|v| v.as_str()).unwrap_or("");
                                let price_opt = item.get("c")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .or_else(|| item.get("c").and_then(|v| v.as_f64()));
                                if let Some(p) = price_opt {
                                    if let Some((base, quote)) = split_symbol_generic(symbol) {
                                        if p > 0.0 {
                                            seen.insert(symbol.to_string(), PairPrice{ base, quote, price: p, is_spot: true });
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if msg.is_ping() {
                    let _ = write.send(Message::Pong(vec![])).await;
                }
            }
            _ => {}
        }
    }

    let _ = write.send(Message::Close(None)).await;
    Ok(seen.into_iter().map(|(_,v)| v).collect())
}

/// fetch bybit spot tickers one-shot
pub async fn fetch_bybit(collect_seconds: Option<u64>) -> Result<Vec<PairPrice>, String> {
    let url = "wss://stream.bybit.com/v5/public/spot";
    info!("bybit: connecting {}", url);
    let ws_url = Url::parse(url).map_err(|e| format!("bybit url parse: {}", e))?;
    let timeout = Duration::from_secs(12);

    let (ws, _) = tokio::time::timeout(timeout, connect_async(ws_url))
        .await
        .map_err(|_| "bybit connect timeout".to_string())?
        .map_err(|e| format!("bybit connect error: {}", e))?;

    let (mut write, mut read) = ws.split();
    // subscribe to tickers
    let sub = serde_json::json!({ "op": "subscribe", "args": ["tickers"] });
    let _ = write.send(Message::Text(sub.to_string())).await.map_err(|e| format!("bybit sub err: {}", e))?;

    let collect = collect_seconds.unwrap_or(DEFAULT_COLLECT_SECONDS);
    let end = Instant::now() + Duration::from_secs(collect);
    let mut seen: HashMap<String, PairPrice> = HashMap::new();

    while Instant::now() < end {
        match tokio::time::timeout(Duration::from_secs(collect), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if msg.is_text() {
                    if let Ok(txt) = msg.into_text() {
                        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                            if v.get("topic").and_then(|t| t.as_str()) == Some("tickers") {
                                if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                                    for it in arr {
                                        let sym = it.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
                                        let price_opt = it.get("lastPrice").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                                            .or_else(|| it.get("lastPrice").and_then(|p| p.as_f64()));
                                        if let Some(p) = price_opt {
                                            if let Some((base, quote)) = split_symbol_generic(sym) {
                                                if p > 0.0 {
                                                    seen.insert(sym.to_string(), PairPrice{ base, quote, price: p, is_spot: true });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // handle op ping
                            if v.get("op").and_then(|o| o.as_str()) == Some("ping") {
                                let _ = write.send(Message::Text(serde_json::json!({"op":"pong"}).to_string())).await;
                            }
                        }
                    }
                } else if msg.is_ping() {
                    let _ = write.send(Message::Pong(vec![])).await;
                }
            }
            _ => {}
        }
    }

    let _ = write.send(Message::Close(None)).await;
    Ok(seen.into_iter().map(|(_,v)| v).collect())
}

/// fetch kucoin via bullet-public + ws snapshot
pub async fn fetch_kucoin(collect_seconds: Option<u64>) -> Result<Vec<PairPrice>, String> {
    info!("kucoin: requesting bullet-public token");
    let client = Client::new();
    let resp = client.post("https://api.kucoin.com/api/v1/bullet-public")
        .send().await.map_err(|e| format!("kucoin token http err: {}", e))?;
    let token_json: Value = resp.json().await.map_err(|e| format!("kucoin token decode err: {}", e))?;
    let token = token_json["data"]["token"].as_str().unwrap_or_default().to_string();
    let endpoint = token_json["data"]["instanceServers"].get(0)
        .and_then(|i| i.get("endpoint")).and_then(|v| v.as_str())
        .unwrap_or("wss://ws-api-spot.kucoin.com").to_string();

    if token.is_empty() {
        return Err("kucoin missing token".to_string());
    }

    let url = format!("{}?token={}", endpoint, token);
    info!("kucoin: connecting {}", url);
    let ws_url = Url::parse(&url).map_err(|e| format!("kucoin url parse: {}", e))?;
    let timeout = Duration::from_secs(14);

    let (ws, _) = tokio::time::timeout(timeout, connect_async(ws_url))
        .await
        .map_err(|_| "kucoin connect timeout".to_string())?
        .map_err(|e| format!("kucoin connect error: {}", e))?;

    let (mut write, mut read) = ws.split();
    let sub = serde_json::json!({
        "id":"scanner",
        "type":"subscribe",
        "topic":"/market/ticker:all",
        "response": true
    });
    let _ = write.send(Message::Text(sub.to_string())).await.map_err(|e| format!("kucoin subscribe err: {}", e))?;

    let collect = collect_seconds.unwrap_or(DEFAULT_COLLECT_SECONDS);
    let end = Instant::now() + Duration::from_secs(collect);
    let mut seen: HashMap<String, PairPrice> = HashMap::new();

    while Instant::now() < end {
        match tokio::time::timeout(Duration::from_secs(collect), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if msg.is_text() {
                    if let Ok(txt) = msg.into_text() {
                        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                            if let Some(data) = v.get("data") {
                                if let (Some(sym_v), Some(price_v)) = (data.get("symbol"), data.get("price")) {
                                    if let (Some(sym), Some(price_str)) = (sym_v.as_str(), price_v.as_str()) {
                                        if let Ok(p) = price_str.parse::<f64>() {
                                            if p > 0.0 {
                                                if let Some((base, quote)) = split_symbol_generic(sym) {
                                                    seen.insert(sym.to_string(), PairPrice{ base, quote, price: p, is_spot: true });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if msg.is_ping() {
                    let _ = write.send(Message::Pong(vec![])).await;
                }
            }
            _ => {}
        }
    }

    let _ = write.send(Message::Close(None)).await;
    Ok(seen.into_iter().map(|(_,v)| v).collect())
}

/// fetch gateio snapshot via WS
pub async fn fetch_gateio(collect_seconds: Option<u64>) -> Result<Vec<PairPrice>, String> {
    let url = "wss://api.gateio.ws/ws/v4/";
    info!("gateio: connecting {}", url);
    let ws_url = Url::parse(url).map_err(|e| format!("gateio url parse: {}", e))?;
    let timeout = Duration::from_secs(12);

    let (ws, _) = tokio::time::timeout(timeout, connect_async(ws_url))
        .await
        .map_err(|_| "gateio connect timeout".to_string())?
        .map_err(|e| format!("gateio connect error: {}", e))?;

    let (mut write, mut read) = ws.split();
    let sub = serde_json::json!({
        "time": chrono::Utc::now().timestamp_millis(),
        "channel": "spot.tickers",
        "event": "subscribe",
        "payload": []
    });
    let _ = write.send(Message::Text(sub.to_string())).await.map_err(|e| format!("gateio subscribe err: {}", e))?;

    let collect = collect_seconds.unwrap_or(DEFAULT_COLLECT_SECONDS);
    let end = Instant::now() + Duration::from_secs(collect);
    let mut seen: HashMap<String, PairPrice> = HashMap::new();

    while Instant::now() < end {
        match tokio::time::timeout(Duration::from_secs(collect), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if msg.is_text() {
                    if let Ok(txt) = msg.into_text() {
                        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                            if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                                if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                                    for it in arr {
                                        if let (Some(sym), Some(last)) = (it.get("currency_pair").and_then(|s| s.as_str()), it.get("last").and_then(|n| n.as_f64())) {
                                            if last > 0.0 {
                                                if let Some((base, quote)) = split_symbol_generic(sym) {
                                                    seen.insert(sym.to_string(), PairPrice{ base, quote, price: last, is_spot: true });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if msg.is_ping() {
                    let _ = write.send(Message::Pong(vec![])).await;
                }
            }
            _ => {}
        }
    }

    let _ = write.send(Message::Close(None)).await;
    Ok(seen.into_iter().map(|(_,v)| v).collect())
          }
