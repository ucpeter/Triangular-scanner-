// src/exchanges.rs
use crate::models::PairPrice;
use futures_util::{StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async};
use tracing::{info, warn, error};

/// Collect a snapshot of Binance (WS-only) tickers over `seconds` seconds.
/// Returns Vec<PairPrice> where each pair is the latest seen for that symbol.
/// This function keeps only the latest price+volume per pair (dedup by symbol).
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
                        // parse an array of tickers
                        if let Ok(txt) = m.into_text() {
                            if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&txt) {
                                for it in arr {
                                    // symbol and last price are typical fields: "s" and "c"
                                    let sym = it.get("s").and_then(|v| v.as_str());
                                    let price_opt = it
                                        .get("c")
                                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                                        .and_then(|s| s.parse::<f64>().ok())
                                        .or_else(|| it.get("c").and_then(|v| v.as_f64()));
                                    // volume fields - try a few common names
                                    let vol_opt = it.get("v")
                                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                                        .and_then(|s| s.parse::<f64>().ok())
                                        .or_else(|| it.get("v").and_then(|v| v.as_f64()))
                                        .or_else(|| it.get("q").and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok())))
                                        .or_else(|| it.get("Q").and_then(|v| v.as_f64()));

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
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("binance ws read error: {:?}", e);
                        break;
                    }
                }
            }
        }
        Err(e) => {
            error!("binance connect error: {:?}", e);
        }
    }

    let pairs: Vec<PairPrice> = out_map.into_values().collect();
    info!("scan complete for binance: collected {} unique pairs", pairs.len());
    pairs
}

/// Wrapper so routes.rs can call collect_exchange_snapshot(exchange, seconds)
pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    match exchange.to_lowercase().as_str() {
        "binance" => collect_binance_snapshot(seconds).await,
        other => {
            warn!("collect_exchange_snapshot: only Binance WS is active (asked for '{}')", other);
            Vec::new()
        }
    }
}

/// Dynamically attempt to split symbol into base/quote.
/// Tries known quotes first; falls back to taking last 3/4 chars if none match.
fn dynamic_split_symbol(sym: &str) -> Option<(String, String)> {
    let s = sym.to_uppercase();
    // common quote suffixes on Binance; expand as needed
    const QUOTES: [&str; 24] = [
        "USDT","BUSD","USDC","FDUSD","TUSD","BTC","ETH","BNB","TRY","EUR","GBP","AUD","BRL","CAD",
        "ARS","RUB","ZAR","NGN","UAH","IDR","JPY","KRW","VND","MXN",
    ];
    for q in &QUOTES {
        if s.ends_with(q) && s.len() > q.len() {
            let base = s[..s.len() - q.len()].to_string();
            return Some((base, q.to_string()));
        }
    }
    // fallback: try splitting last 3 or 4 chars if symbol long enough
    if s.len() > 6 {
        let try3 = s.split_at(s.len() - 3);
        // Basic sanity: quote must be alphabetic
        if try3.1.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some((try3.0.to_string(), try3.1.to_string()));
        }
    }
    None
                                                                     }
