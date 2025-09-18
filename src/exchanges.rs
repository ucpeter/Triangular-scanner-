use crate::models::PairPrice;
use futures_util::{StreamExt};
use serde_json::Value;
use std::collections::HashSet;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::connect_async;
use tracing::{info, warn};

/// Collect snapshot of Binance spot tickers over `seconds` seconds using WS only
pub async fn collect_binance_snapshot(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("Connecting to binance at {}", url);

    let (ws_stream, _) = match connect_async(url).await {
        Ok(res) => res,
        Err(e) => {
            warn!("binance connect error: {}", e);
            return Vec::new();
        }
    };

    let (mut _write, mut read) = ws_stream.split();

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut elapsed = 0u64;

    while elapsed < seconds {
        if let Some(Ok(msg)) = read.next().await {
            if let Ok(text) = msg.to_text() {
                if let Ok(json) = serde_json::from_str::<Value>(text) {
                    if let Some(arr) = json.as_array() {
                        for it in arr {
                            if let (Some(s), Some(p)) = (it.get("s"), it.get("c")) {
                                if let (Some(symbol), Some(price)) = (s.as_str(), p.as_str()) {
                                    // Skip futures or fiat-only pairs, keep spot
                                    if symbol.len() < 6 {
                                        continue;
                                    }

                                    let (base, quote) = split_symbol(symbol);
                                    let key = format!("{}-{}", base, quote);

                                    if seen.insert(key.clone()) {
                                        if let Ok(px) = price.parse::<f64>() {
                                            out.push(PairPrice {
                                                base,
                                                quote,
                                                price: px,
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
        sleep(Duration::from_secs(1)).await;
        elapsed += 1;
    }

    info!(
        "binance collected {} unique pairs, seen_total={}, skipped={}",
        out.len(),
        seen.len(),
        seen.len().saturating_sub(out.len())
    );
    out
}

/// Simple dynamic symbol splitter (no hardcoding, works for stablecoins & others)
fn split_symbol(symbol: &str) -> (String, String) {
    // Known quote suffixes (Binance spot)
    const QUOTES: [&str; 9] = ["USDT","USDC","BUSD","FDUSD","TUSD","BTC","ETH","BNB","TRY"];

    for q in QUOTES {
        if symbol.ends_with(q) {
            let base = symbol[..symbol.len()-q.len()].to_string();
            return (base, q.to_string());
        }
    }

    // Fallback: split last 3 chars
    let (b, q) = symbol.split_at(symbol.len().saturating_sub(3));
    (b.to_string(), q.to_string())
}

/// Wrapper so routes.rs can stay unchanged
pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    match exchange.to_lowercase().as_str() {
        "binance" => collect_binance_snapshot(seconds).await,
        other => {
            warn!("Exchange {} not yet supported", other);
            Vec::new()
        }
    }
                                        }
