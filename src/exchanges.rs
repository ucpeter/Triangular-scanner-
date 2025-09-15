use crate::models::PairPrice;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

/// Public entry: collects a snapshot for `exchange` over `seconds`.
/// Returns a Vec<PairPrice> (base, quote, price, is_spot=true).
pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    match exchange.to_lowercase().as_str() {
        "binance" => collect_binance_snapshot(seconds).await,
        "bybit" => collect_bybit_snapshot(seconds).await,
        "gateio" => collect_gateio_snapshot(seconds).await,
        "kucoin" => collect_kucoin_snapshot(seconds).await,
        other => {
            warn!("collect_snapshot: unknown exchange {}, defaulting to empty", other);
            Vec::new()
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// ----------------------------- BINANCE ------------------------------------
////////////////////////////////////////////////////////////////////////////////

async fn fetch_binance_symbol_map() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let url = "https://api.binance.com/api/v3/exchangeInfo";
    match Client::new().get(url).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(json) => {
                if let Some(arr) = json.get("symbols").and_then(|v| v.as_array()) {
                    for s in arr {
                        if let (Some(sym), Some(base), Some(quote)) = (
                            s.get("symbol").and_then(|v| v.as_str()),
                            s.get("baseAsset").and_then(|v| v.as_str()),
                            s.get("quoteAsset").and_then(|v| v.as_str()),
                        ) {
                            map.insert(sym.to_string(), (base.to_string(), quote.to_string()));
                        }
                    }
                }
            }
            Err(e) => {
                error!("binance: failed parse exchangeInfo JSON: {}", e);
            }
        },
        Err(e) => {
            error!("binance: failed fetch exchangeInfo: {}", e);
        }
    }
    map
}

async fn collect_binance_snapshot(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("Connecting to binance at {}", url);

    let symbol_map = fetch_binance_symbol_map().await;
    if symbol_map.is_empty() {
        warn!("binance: symbol map empty; dynamic splitting may fail for some symbols");
    }

    let mut out: Vec<PairPrice> = Vec::new();
    let mut seen_total = 0usize;
    let mut seen_unique = 0usize;
    let mut skipped = 0usize;

    match connect_async(url).await {
        Ok((ws_stream, _)) => {
            let (_write, mut read) = ws_stream.split();
            let deadline = Instant::now() + Duration::from_secs(seconds);

            // We'll poll until deadline; parse arrays of tickers from the stream.
            while Instant::now() < deadline {
                match tokio::time::timeout(Duration::from_secs( (seconds.min(5)).max(1) ), read.next()).await {
                    Ok(Some(Ok(msg))) => {
                        if msg.is_text() {
                            if let Ok(txt) = msg.into_text() {
                                // Binance sends an array of objects on !ticker@arr
                                if let Ok(js) = serde_json::from_str::<Value>(&txt) {
                                    if let Value::Array(arr) = js {
                                        for it in arr {
                                            seen_total += 1;
                                            // symbol fields: "s", price: "c"
                                            if let (Some(sym), Some(price_v)) = (
                                                it.get("s").and_then(|v| v.as_str()),
                                                it.get("c"),
                                            ) {
                                                // parse price robustly
                                                let price_opt = price_v.as_str().and_then(|s| s.parse::<f64>().ok())
                                                    .or_else(|| price_v.as_f64());
                                                if let Some(price) = price_opt {
                                                    if price > 0.0 {
                                                        if let Some((base, quote)) = symbol_map.get(sym) {
                                                            out.push(PairPrice {
                                                                base: base.to_string(),
                                                                quote: quote.to_string(),
                                                                price,
                                                                is_spot: true,
                                                            });
                                                            seen_unique += 1;
                                                        } else {
                                                            skipped += 1;
                                                            // fallback attempt: try to split by known suffixes as a last resort
                                                            if let Some((b, q)) = fallback_split(sym) {
                                                                out.push(PairPrice { base: b, quote: q, price, is_spot: true });
                                                                seen_unique += 1;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Some(Err(e))) => {
                        error!("binance ws read error: {:?}", e);
                        break;
                    }
                    Ok(None) => break, // stream ended
                    Err(_) => {
                        // timeout waiting for a message; continue until deadline
                        continue;
                    }
                }
            }
        }
        Err(e) => {
            error!("binance connect error: {:?}", e);
        }
    }

    info!("binance collected {} unique pairs, seen_total={}, skipped={}", seen_unique, seen_total, skipped);
    out
}

////////////////////////////////////////////////////////////////////////////////
// ----------------------------- BYBIT --------------------------------------
////////////////////////////////////////////////////////////////////////////////

async fn fetch_bybit_symbol_map() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let url = "https://api.bybit.com/v5/market/instruments-info?category=spot";
    match Client::new().get(url).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(json) => {
                if let Some(arr) = json.get("result").and_then(|v| v.get("list")).and_then(|v| v.as_array()) {
                    for it in arr {
                        if let (Some(sym), Some(base), Some(quote), Some(status)) = (
                            it.get("symbol").and_then(|v| v.as_str()),
                            it.get("baseCoin").and_then(|v| v.as_str()),
                            it.get("quoteCoin").and_then(|v| v.as_str()),
                            it.get("status").and_then(|v| v.as_str()),
                        ) {
                            // keep only trading pairs
                            if status.eq_ignore_ascii_case("Trading") {
                                map.insert(sym.to_string(), (base.to_string(), quote.to_string()));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("bybit: failed parse instruments-info: {}", e);
            }
        },
        Err(e) => {
            error!("bybit: failed fetch instruments-info: {}", e);
        }
    }
    map
}

async fn collect_bybit_snapshot(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://stream.bybit.com/v5/public/spot";
    info!("Connecting to bybit at {}", url);

    let symbol_map = fetch_bybit_symbol_map().await;
    if symbol_map.is_empty() {
        warn!("bybit: symbol map empty; pairs may be skipped");
    }

    let mut out = Vec::new();
    let mut seen_total = 0usize;
    let mut seen_unique = 0usize;
    let mut skipped = 0usize;

    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            // subscribe to tickers topic
            let sub = serde_json::json!({
                "op": "subscribe",
                "args": ["tickers"]
            });
            let _ = ws_stream.send(Message::Text(sub.to_string())).await;

            let (_write, mut read) = ws_stream.split();
            let deadline = Instant::now() + Duration::from_secs(seconds);

            while Instant::now() < deadline {
                match tokio::time::timeout(Duration::from_secs( (seconds.min(5)).max(1) ), read.next()).await {
                    Ok(Some(Ok(msg))) => {
                        if msg.is_text() {
                            if let Ok(txt) = msg.into_text() {
                                if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                    // Bybit uses { "topic": "tickers", "data": [ ... ] }
                                    if v.get("topic").and_then(|t| t.as_str()) == Some("tickers") {
                                        if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                                            for it in arr {
                                                seen_total += 1;
                                                if let (Some(sym), Some(price_v)) = (
                                                    it.get("symbol").and_then(|s| s.as_str()),
                                                    it.get("lastPrice").or_else(|| it.get("last_price"))
                                                ) {
                                                    let price_opt = price_v.as_str().and_then(|s| s.parse::<f64>().ok())
                                                        .or_else(|| price_v.as_f64());
                                                    if let Some(price) = price_opt {
                                                        if price > 0.0 {
                                                            if let Some((base, quote)) = symbol_map.get(sym) {
                                                                out.push(PairPrice { base: base.clone(), quote: quote.clone(), price, is_spot: true });
                                                                seen_unique += 1;
                                                            } else {
                                                                skipped += 1;
                                                                if let Some((b,q)) = fallback_split(sym) {
                                                                    out.push(PairPrice { base: b, quote: q, price, is_spot: true });
                                                                    seen_unique += 1;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Some(Err(e))) => {
                        error!("bybit ws read error: {:?}", e);
                        break;
                    }
                    Ok(None) => break,
                    Err(_) => continue,
                }
            }
        }
        Err(e) => {
            error!("bybit connect error: {:?}", e);
        }
    }

    info!("bybit collected {} unique pairs, seen_total={}, skipped={}", seen_unique, seen_total, skipped);
    out
}

////////////////////////////////////////////////////////////////////////////////
// ----------------------------- GATE.IO ------------------------------------
////////////////////////////////////////////////////////////////////////////////

async fn fetch_gateio_symbol_map() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let url = "https://api.gateio.ws/api/v4/spot/currency_pairs";
    match Client::new().get(url).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(json) => {
                if let Some(arr) = json.as_array() {
                    for it in arr {
                        if let (Some(id), Some(base), Some(quote)) = (
                            it.get("id").and_then(|v| v.as_str()),
                            it.get("base").and_then(|v| v.as_str()),
                            it.get("quote").and_then(|v| v.as_str()),
                        ) {
                            // id is like "BTC_USDT" (uppercase)
                            map.insert(id.to_uppercase(), (base.to_string(), quote.to_string()));
                            // also add without underscore for WS convenience
                            map.insert(id.replace('_', "").to_uppercase(), (base.to_string(), quote.to_string()));
                        }
                    }
                }
            }
            Err(e) => {
                error!("gateio: failed parse currency_pairs: {}", e);
            }
        },
        Err(e) => {
            error!("gateio: failed fetch currency_pairs: {}", e);
        }
    }
    map
}

async fn collect_gateio_snapshot(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://api.gateio.ws/ws/v4/";
    info!("Connecting to gateio at {}", url);

    let symbol_map = fetch_gateio_symbol_map().await;
    if symbol_map.is_empty() {
        warn!("gateio: symbol map empty; pairs may be skipped");
    }

    let mut out = Vec::new();
    let mut seen_total = 0usize;
    let mut seen_unique = 0usize;
    let mut skipped = 0usize;

    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            // subscribe to spot.tickers channel
            let sub = serde_json::json!({
                "time": chrono::Utc::now().timestamp_millis(),
                "channel":"spot.tickers",
                "event":"subscribe",
                "payload":[]
            });
            let _ = ws_stream.send(Message::Text(sub.to_string())).await;

            let (_write, mut read) = ws_stream.split();
            let deadline = Instant::now() + Duration::from_secs(seconds);

            while Instant::now() < deadline {
                match tokio::time::timeout(Duration::from_secs( (seconds.min(5)).max(1) ), read.next()).await {
                    Ok(Some(Ok(msg))) => {
                        if msg.is_text() {
                            if let Ok(txt) = msg.into_text() {
                                if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                    // gateio ticker messages: { "channel":"spot.tickers", "result": [...] }
                                    if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                                        if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                                            for it in arr {
                                                seen_total += 1;
                                                if let (Some(sym_v), Some(last_v)) = (it.get("currency_pair"), it.get("last")) {
                                                    if let (Some(sym), Some(last_f)) = (sym_v.as_str(), last_v.as_f64()) {
                                                        if last_f > 0.0 {
                                                            // support both "BTC_USDT" and "BTCUSDT" keys
                                                            let key1 = sym.to_uppercase();
                                                            let key2 = sym.replace('_', "").to_uppercase();
                                                            if let Some((base, quote)) = symbol_map.get(&key1).or_else(|| symbol_map.get(&key2)) {
                                                                out.push(PairPrice { base: base.clone(), quote: quote.clone(), price: last_f, is_spot: true });
                                                                seen_unique += 1;
                                                            } else {
                                                                skipped += 1;
                                                                if let Some((b,q)) = fallback_split(sym) {
                                                                    out.push(PairPrice { base: b, quote: q, price: last_f, is_spot: true });
                                                                    seen_unique += 1;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Some(Err(e))) => {
                        error!("gateio ws read error: {:?}", e);
                        break;
                    }
                    Ok(None) => break,
                    Err(_) => continue,
                }
            }
        }
        Err(e) => {
            error!("gateio connect error: {:?}", e);
        }
    }

    info!("gateio collected {} unique pairs, seen_total={}, skipped={}", seen_unique, seen_total, skipped);
    out
}

////////////////////////////////////////////////////////////////////////////////
// ----------------------------- KUCOIN -------------------------------------
////////////////////////////////////////////////////////////////////////////////

async fn fetch_kucoin_symbol_map() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let url = "https://api.kucoin.com/api/v1/symbols";
    match Client::new().get(url).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(json) => {
                if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
                    for it in arr {
                        if let (Some(sym), Some(base), Some(quote), Some(enable)) = (
                            it.get("symbol").and_then(|v| v.as_str()),
                            it.get("baseCurrency").and_then(|v| v.as_str()),
                            it.get("quoteCurrency").and_then(|v| v.as_str()),
                            it.get("enableTrading"),
                        ) {
                            // only keep enabled/tradable
                            let enabled = enable.as_bool().unwrap_or(false);
             
