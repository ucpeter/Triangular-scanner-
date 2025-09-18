use crate::models::PairPrice;
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tracing::{info, warn, error};

/// Collect a snapshot of Binance (WS-only) tickers over `seconds` seconds.
/// Returns Vec<PairPrice> where each pair is the latest seen for that symbol.
pub async fn collect_binance_snapshot(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("Connecting to Binance WS at {}", url);

    let mut out_map: HashMap<String, PairPrice> = HashMap::new();

    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            let deadline = Instant::now() + Duration::from_secs(seconds);

            while let Some(msg) = ws_stream.next().await {
                if Instant::now() >= deadline {
                    break;
                }

                match msg {
                    Ok(m) if m.is_text() => {
                        if let Ok(txt) = m.into_text() {
                            match serde_json::from_str::<Value>(&txt) {
                                Ok(Value::Array(arr)) => {
                                    for it in arr {
                                        let sym = it.get("s").and_then(|v| v.as_str());
                                        let price_opt = parse_f64(it.get("c"));
                                        let vol_opt = parse_f64(it.get("v"))
                                            .or_else(|| parse_f64(it.get("q")))
                                            .or_else(|| parse_f64(it.get("Q")));

                                        if let (Some(sym), Some(price)) = (sym, price_opt) {
                                            if let Some((base, quote)) = dynamic_split_symbol(sym) {
                                                let vol = vol_opt.unwrap_or(0.0);
                                                let key = format!("{}/{}", base, quote);
                                                out_map.insert(
                                                    key.clone(),
                                                    PairPrice {
                                                        base,
                                                        quote,
                                                        price,
                                                        is_spot: true,
                                                        volume: vol,
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(_) => warn!("Failed to parse Binance WS message: {}", txt),
                                _ => {}
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("binance ws read error: {:?}", e);
                        break;
                    }
                }

                // prevent tight CPU loop
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        Err(e) => {
            error!("binance connect error: {:?}", e);
        }
    }

    let pairs: Vec<PairPrice> = out_map.into_values().collect();
    info!(
        "scan complete for binance: collected {} unique pairs",
        pairs.len()
    );
    pairs
}

/// Wrapper so routes.rs can call collect_exchange_snapshot(exchange, seconds)
pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    match exchange.to_lowercase().as_str() {
        "binance" => collect_binance_snapshot(seconds).await,
        other => {
            warn!(
                "collect_exchange_snapshot: only Binance WS is active (asked for '{}')",
                other
            );
            Vec::new()
        }
    }
}

/// Try to split symbol into base/quote.
fn dynamic_split_symbol(sym: &str) -> Option<(String, String)> {
    let s = sym.to_uppercase();
    const QUOTES: [&str; 24] = [
        "USDT", "BUSD", "USDC", "FDUSD", "TUSD", "BTC", "ETH", "BNB", "TRY", "EUR", "GBP", "AUD",
        "BRL", "CAD", "ARS", "RUB", "ZAR", "NGN", "UAH", "IDR", "JPY", "KRW", "VND", "MXN",
    ];

    for q in &QUOTES {
        if s.ends_with(q) && s.len() > q.len() {
            let base = s[..s.len() - q.len()].to_string();
            return Some((base, q.to_string()));
        }
    }

    if s.len() > 6 {
        let try3 = s.split_at(s.len() - 3);
        if try3.1.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some((try3.0.to_string(), try3.1.to_string()));
        }
    }
    if s.len() > 7 {
        let try4 = s.split_at(s.len() - 4);
        if try4.1.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some((try4.0.to_string(), try4.1.to_string()));
        }
    }
    None
}

/// Helper: parse f64 from JSON value
fn parse_f64(v: Option<&Value>) -> Option<f64> {
    v.and_then(|val| val.as_f64().or_else(|| val.as_str()?.parse::<f64>().ok()))
                                        }
