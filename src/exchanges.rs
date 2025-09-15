use crate::models::PairPrice;
use tokio::time::{Duration, Instant};
use tracing::{info, error, warn};
use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use serde_json::Value;
use std::collections::HashMap;


/// REST metadata fetcher for Binance (base/quote map)
async fn fetch_binance_symbols() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();

    if let Ok(resp) = reqwest::get("https://api.binance.com/api/v3/exchangeInfo").await {
        if let Ok(json) = resp.json::<Value>().await {
            if let Some(arr) = json.get("symbols").and_then(|v| v.as_array()) {
                for s in arr {
                    if let (Some(sym), Some(base), Some(quote)) = (
                        s.get("symbol").and_then(|x| x.as_str()),
                        s.get("baseAsset").and_then(|x| x.as_str()),
                        s.get("quoteAsset").and_then(|x| x.as_str()),
                    ) {
                        map.insert(sym.to_string(), (base.to_string(), quote.to_string()));
                    }
                }
            }
        }
    }

    map
}

/// Collects a snapshot of market prices for a given exchange over a few seconds.
pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    let url = match exchange {
        "binance" => "wss://stream.binance.com:9443/ws/!ticker@arr",
        _ => {
            warn!("Unknown exchange {}, defaulting to Binance", exchange);
            "wss://stream.binance.com:9443/ws/!ticker@arr"
        }
    };

    info!("Connecting to {} at {}", exchange, url);
    let mut pairs: Vec<PairPrice> = Vec::new();

    // preload metadata for base/quote splits
    let symbol_map = if exchange == "binance" {
        fetch_binance_symbols().await
    } else {
        HashMap::new()
    };

    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            let deadline = Instant::now() + Duration::from_secs(seconds);

            while let Some(msg) = ws_stream.next().await {
                if Instant::now() >= deadline {
                    break;
                }

                if let Ok(m) = msg {
                    if m.is_text() {
                        if let Ok(txt) = m.into_text() {
                            if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                if exchange == "binance" {
                                    if let Value::Array(arr) = v {
                                        for it in arr {
                                            if let (Some(sym), Some(price)) = (
                                                it.get("s").and_then(|x| x.as_str()),
                                                it.get("c").and_then(|x| x.as_str())
                                                    .and_then(|s| s.parse::<f64>().ok())
                                            ) {
                                                if price > 0.0 {
                                                    if let Some((base, quote)) = symbol_map.get(sym) {
                                                        pairs.push(PairPrice {
                                                            base: base.clone(),
                                                            quote: quote.clone(),
                                                            price,
                                                            is_spot: true,
                                                        });
                                                    } else {
                                                        warn!("{}: missing symbol map for {}", exchange, sym);
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
        Err(e) => {
            error!("{} connect error: {:?}", exchange, e);
        }
    }

    info!("{} collected {} pairs", exchange, pairs.len());
    pairs
                                                    }
