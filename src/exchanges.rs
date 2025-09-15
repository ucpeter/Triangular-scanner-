use crate::models::PairPrice;
use futures_util::{StreamExt, SinkExt};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info, warn};

pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    match exchange {
        "binance" => collect_binance(seconds).await,
        "bybit" => collect_bybit(seconds).await,
        "gateio" => collect_gateio(seconds).await,
        "kucoin" => collect_kucoin(seconds).await,
        _ => Vec::new(),
    }
}

async fn collect_binance(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    let mut pairs: Vec<PairPrice> = Vec::new();
    let mut symbol_map = fetch_binance_symbol_map().await;

    info!("Connecting to binance at {}", url);

    match connect_async(url).await {
        Ok((mut ws, _)) => {
            let deadline = Instant::now() + Duration::from_secs(seconds);

            while let Some(msg) = ws.next().await {
                if Instant::now() >= deadline {
                    break;
                }

                if let Ok(m) = msg {
                    if m.is_text() {
                        if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&m.into_text().unwrap()) {
                            for it in arr {
                                if let (Some(sym), Some(price)) = (
                                    it.get("s").and_then(|v| v.as_str()),
                                    it.get("c").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()),
                                ) {
                                    if let Some((base, quote)) = symbol_map.get(sym) {
                                        pairs.push(PairPrice {
                                            base: base.clone(),
                                            quote: quote.clone(),
                                            price,
                                            is_spot: true,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("binance connect error: {:?}", e);
        }
    }

    info!("binance collected {} pairs", pairs.len());
    pairs
}

async fn collect_bybit(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://stream.bybit.com/v5/public/spot";
    let mut pairs: Vec<PairPrice> = Vec::new();
    let symbol_map = fetch_bybit_symbol_map().await;

    info!("Connecting to bybit at {}", url);

    match connect_async(url).await {
        Ok((mut ws, _)) => {
            let sub = serde_json::json!({ "op": "subscribe", "args": ["tickers"] });
            let _ = ws.send(Message::Text(sub.to_string())).await;

            let deadline = Instant::now() + Duration::from_secs(seconds);

            while let Some(msg) = ws.next().await {
                if Instant::now() >= deadline {
                    break;
                }

                if let Ok(m) = msg {
                    if m.is_text() {
                        if let Ok(v) = serde_json::from_str::<Value>(&m.into_text().unwrap()) {
                            if v.get("topic").and_then(|t| t.as_str()) == Some("tickers") {
                                if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                                    for it in arr {
                                        if let (Some(sym), Some(price)) = (
                                            it.get("symbol").and_then(|s| s.as_str()),
                                            it.get("lastPrice").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok()),
                                        ) {
                                            if let Some((base, quote)) = symbol_map.get(sym) {
                                                pairs.push(PairPrice {
                                                    base: base.clone(),
                                                    quote: quote.clone(),
                                                    price,
                                                    is_spot: true,
                                                });
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
        Err(e) => {
            error!("bybit connect error: {:?}", e);
        }
    }

    info!("bybit collected {} pairs", pairs.len());
    pairs
}

async fn collect_gateio(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://api.gateio.ws/ws/v4/";
    let mut pairs: Vec<PairPrice> = Vec::new();
    let symbol_map = fetch_gateio_symbol_map().await;

    info!("Connecting to gateio at {}", url);

    match connect_async(url).await {
        Ok((mut ws, _)) => {
            let sub = serde_json::json!({
                "time": chrono::Utc::now().timestamp_millis(),
                "channel":"spot.tickers",
                "event":"subscribe",
                "payload":[]
            });
            let _ = ws.send(Message::Text(sub.to_string())).await;

            let deadline = Instant::now() + Duration::from_secs(seconds);

            while let Some(msg) = ws.next().await {
                if Instant::now() >= deadline {
                    break;
                }

                if let Ok(m) = msg {
                    if m.is_text() {
                        if let Ok(v) = serde_json::from_str::<Value>(&m.into_text().unwrap()) {
                            if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
                                if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                                    for it in arr {
                                        if let (Some(sym), Some(price)) = (
                                            it.get("currency_pair").and_then(|s| s.as_str()),
                                            it.get("last").and_then(|s| s.as_f64()),
                                        ) {
                                            if let Some((base, quote)) = symbol_map.get(sym) {
                                                pairs.push(PairPrice {
                                                    base: base.clone(),
                                                    quote: quote.clone(),
                                                    price,
                                                    is_spot: true,
                                                });
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
        Err(e) => {
            error!("gateio connect error: {:?}", e);
        }
    }

    info!("gateio collected {} pairs", pairs.len());
    pairs
}

async fn collect_kucoin(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://ws-api-spot.kucoin.com/?token=public";
    let mut pairs: Vec<PairPrice> = Vec::new();
    let symbol_map = fetch_kucoin_symbol_map().await;

    info!("Connecting to kucoin at {}", url);

    match connect_async(url).await {
        Ok((mut ws, _)) => {
            let sub = serde_json::json!({
                "id":"scanner",
                "type":"subscribe",
                "topic":"/market/ticker:all",
                "response": true
            });
            let _ = ws.send(Message::Text(sub.to_string())).await;

            let deadline = Instant::now() + Duration::from_secs(seconds);

            while let Some(msg) = ws.next().await {
                if Instant::now() >= deadline {
                    break;
                }

                if let Ok(m) = msg {
                    if m.is_text() {
                        if let Ok(v) = serde_json::from_str::<Value>(&m.into_text().unwrap()) {
                            if let Some(data) = v.get("data") {
                                if let (Some(sym), Some(price_str)) =
                                    (data.get("symbol").and_then(|s| s.as_str()), data.get("price").and_then(|p| p.as_str()))
                                {
                                    if let Ok(price) = price_str.parse::<f64>() {
                                        if let Some((base, quote)) = symbol_map.get(sym) {
                                            pairs.push(PairPrice {
                                                base: base.clone(),
                                                quote: quote.clone(),
                                                price,
                                                is_spot: true,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("kucoin connect error: {:?}", e);
        }
    }

    info!("kucoin collected {} pairs", pairs.len());
    pairs
}

async fn fetch_binance_symbol_map() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let url = "https://api.binance.com/api/v3/exchangeInfo";

    match Client::new().get(url).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(json) => {
                if let Some(arr) = json.get("symbols").and_then(|v| v.as_array()) {
                    for it in arr {
                        if let (Some(sym), Some(base), Some(quote), Some(status)) = (
                            it.get("symbol").and_then(|v| v.as_str()),
                            it.get("baseAsset").and_then(|v| v.as_str()),
                            it.get("quoteAsset").and_then(|v| v.as_str()),
                            it.get("status").and_then(|v| v.as_str()),
                        ) {
                            if status == "TRADING" {
                                map.insert(sym.to_string(), (base.to_string(), quote.to_string()));
                            }
                        }
                    }
                }
            }
            Err(e) => error!("binance: failed parse symbols: {}", e),
        },
        Err(e) => error!("binance: failed fetch symbols: {}", e),
    }

    map
}

async fn fetch_bybit_symbol_map() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let url = "https://api.bybit.com/v5/market/instruments-info?category=spot";

    match Client::new().get(url).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(json) => {
                if let Some(arr) = json.get("result").and_then(|r| r.get("list")).and_then(|v| v.as_array()) {
                    for it in arr {
                        if let (Some(sym), Some(base), Some(quote)) = (
                            it.get("symbol").and_then(|v| v.as_str()),
                            it.get("baseCoin").and_then(|v| v.as_str()),
                            it.get("quoteCoin").and_then(|v| v.as_str()),
                        ) {
                            map.insert(sym.to_string(), (base.to_string(), quote.to_string()));
                        }
                    }
                }
            }
            Err(e) => error!("bybit: failed parse symbols: {}", e),
        },
        Err(e) => error!("bybit: failed fetch symbols: {}", e),
    }

    map
}

async fn fetch_gateio_symbol_map() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let url = "https://api.gateio.ws/api/v4/spot/currency_pairs";

    match Client::new().get(url).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(json) => {
                if let Some(arr) = json.as_array() {
                    for it in arr {
                        if let (Some(sym), Some(base), Some(quote), Some(trade_status)) = (
                            it.get("id").and_then(|v| v.as_str()),
                            it.get("base").and_then(|v| v.as_str()),
                            it.get("quote").and_then(|v| v.as_str()),
                            it.get("trade_status").and_then(|v| v.as_str()),
                        ) {
                            if trade_status == "tradable" {
                                map.insert(sym.to_uppercase(), (base.to_string(), quote.to_string()));
                            }
                        }
                    }
                }
            }
            Err(e) => error!("gateio: failed parse symbols: {}", e),
        },
        Err(e) => error!("gateio: failed fetch symbols: {}", e),
    }

    map
}

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
                            let enabled = enable.as_bool().unwrap_or(false);
                            if enabled {
                                map.insert(sym.to_string(), (base.to_string(), quote.to_string()));
                                map.insert(sym.replace('-', "").to_uppercase(), (base.to_string(), quote.to_string()));
                            }
                        }
                    }
                }
            }
            Err(e) => error!("kucoin: failed parse symbols: {}", e),
        },
        Err(e) => error!("kucoin: failed fetch symbols: {}", e),
    }

    map
                            }
