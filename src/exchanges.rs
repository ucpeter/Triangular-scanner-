use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use crate::models::PairPrice;
use tracing::{info, warn, error};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use chrono::Utc;
use url::Url;

/// Collect prices from all exchanges (one-shot scan)
pub async fn collect_prices() -> Vec<PairPrice> {
    let mut out = Vec::new();

    if let Ok(mut b) = fetch_binance().await { out.append(&mut b); }
    if let Ok(mut g) = fetch_gateio().await { out.append(&mut g); }
    if let Ok(mut k) = fetch_kucoin().await { out.append(&mut k); }
    if let Ok(mut y) = fetch_bybit().await { out.append(&mut y); }

    out
}

/// ---------------- Binance ----------------
async fn fetch_binance() -> Result<Vec<PairPrice>, String> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    let (ws_stream, _) = connect_async(url).await.map_err(|e| e.to_string())?;
    let (_, mut read) = ws_stream.split();

    let mut out = Vec::new();
    if let Some(Ok(msg)) = read.next().await {
        if let Ok(txt) = msg.into_text() {
            if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&txt) {
                for item in arr {
                    if let (Some(sym), Some(price)) = (
                        item.get("s").and_then(|s| s.as_str()),
                        item.get("c").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                    ) {
                        let (base, quote) = split_symbol(sym);
                        if !base.is_empty() {
                            out.push(PairPrice { base, quote, price, is_spot: true });
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// ---------------- Bybit ----------------
async fn fetch_bybit() -> Result<Vec<PairPrice>, String> {
    let url = "wss://stream.bybit.com/v5/public/spot";
    let (mut ws, _) = connect_async(url).await.map_err(|e| e.to_string())?;
    let sub = serde_json::json!({"op":"subscribe","args":["tickers"]});
    ws.send(Message::Text(sub.to_string())).await.map_err(|e| e.to_string())?;

    let (_, mut read) = ws.split();
    let mut out = Vec::new();
    if let Some(Ok(msg)) = read.next().await {
        if let Ok(txt) = msg.into_text() {
            if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                if v.get("topic").and_then(|t| t.as_str()) == Some("tickers") {
                    if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                        for it in arr {
                            if let (Some(sym), Some(price)) = (
                                it.get("symbol").and_then(|s| s.as_str()),
                                it.get("lastPrice").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                            ) {
                                let (base, quote) = split_symbol(sym);
                                if !base.is_empty() {
                                    out.push(PairPrice { base, quote, price, is_spot: true });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// ---------------- Gate.io ----------------
async fn fetch_gateio() -> Result<Vec<PairPrice>, String> {
    let url = "wss://api.gateio.ws/ws/v4/";
    let (mut ws, _) = connect_async(url).await.map_err(|e| e.to_string())?;

    let sub_msg = serde_json::json!({
        "time": Utc::now().timestamp_millis(),
        "channel":"spot.tickers",
        "event":"subscribe",
        "payload":[]
    });
    ws.send(Message::Text(sub_msg.to_string())).await.map_err(|e| e.to_string())?;

    let (_, mut read) = ws.split();
    let mut out = Vec::new();
    if let Some(Ok(msg)) = read.next().await {
        if let Ok(txt) = msg.into_text() {
            if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                    if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                        for it in arr {
                            if let (Some(sym), Some(price)) = (
                                it.get("currency_pair").and_then(|s| s.as_str()),
                                it.get("last").and_then(|s| s.as_f64())
                            ) {
                                let parts: Vec<&str> = sym.split('_').collect();
                                if parts.len() == 2 {
                                    out.push(PairPrice { base: parts[0].into(), quote: parts[1].into(), price, is_spot: true });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// ---------------- Kucoin ----------------
async fn fetch_kucoin() -> Result<Vec<PairPrice>, String> {
    let client = reqwest::Client::new();
    let token_resp: Value = client.post("https://api.kucoin.com/api/v1/bullet-public")
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let token = token_resp["data"]["token"].as_str().unwrap_or_default();
    let endpoint = token_resp["data"]["instanceServers"][0]["endpoint"].as_str().unwrap_or("");
    let url = format!("{}?token={}", endpoint, token);

    let (mut ws, _) = connect_async(Url::parse(&url).unwrap()).await.map_err(|e| e.to_string())?;
    let sub = serde_json::json!({"id":"scanner","type":"subscribe","topic":"/market/ticker:all","response":true});
    ws.send(Message::Text(sub.to_string())).await.map_err(|e| e.to_string())?;

    let (_, mut read) = ws.split();
    let mut out = Vec::new();
    if let Some(Ok(msg)) = read.next().await {
        if let Ok(txt) = msg.into_text() {
            if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                if let Some(data) = v.get("data") {
                    if let (Some(sym), Some(price)) = (
                        data.get("symbol").and_then(|s| s.as_str()),
                        data.get("price").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok())
                    ) {
                        let parts: Vec<&str> = sym.split('-').collect();
                        if parts.len() == 2 {
                            out.push(PairPrice { base: parts[0].into(), quote: parts[1].into(), price, is_spot: true });
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// ---------------- Helpers ----------------
fn split_symbol(sym: &str) -> (String, String) {
    let suffixes = ["USDT","BUSD","USDC","BTC","ETH"];
    for s in suffixes {
        if sym.ends_with(s) && sym.len() > s.len() {
            let base = sym.trim_end_matches(s).to_string();
            return (base, s.to_string());
        }
    }
    (String::new(), String::new())
                }
