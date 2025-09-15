use crate::models::PairPrice;
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info};

pub async fn collect_binance_snapshot(seconds: u64) -> Vec<PairPrice> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    let mut pairs: Vec<PairPrice> = Vec::new();

    info!("Connecting to Binance WS-only at {}", url);

    match connect_async(url).await {
        Ok((mut ws, _)) => {
            let deadline = Instant::now() + Duration::from_secs(seconds);
            let mut seen: HashMap<String, PairPrice> = HashMap::new();

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
                                    // Dynamically split symbol into base + quote
                                    if let Some((base, quote)) = dynamic_split_symbol(sym) {
                                        if price > 0.0 {
                                            seen.insert(
                                                sym.to_string(),
                                                PairPrice {
                                                    base,
                                                    quote,
                                                    price,
                                                    is_spot: true,
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            pairs.extend(seen.into_values());
        }
        Err(e) => {
            error!("binance connect error: {:?}", e);
        }
    }

    info!("binance collected {} pairs", pairs.len());
    pairs
}

/// Dynamically split symbols into base + quote (no hardcoded suffixes).
fn dynamic_split_symbol(sym: &str) -> Option<(String, String)> {
    // Try splitting by common stable/crypto bases but without excluding anything
    let known_quotes = [
        "USDT", "BUSD", "USDC", "TUSD", "FDUSD", "BTC", "ETH", "BNB", "TRY", "EUR", "GBP", "AUD",
        "CAD", "JPY", "RUB", "ZAR", "NGN", "BRL", "MXN", "ARS", "AED", "HKD", "SGD",
    ];

    let s = sym.to_uppercase();
    for q in &known_quotes {
        if s.ends_with(q) && s.len() > q.len() {
            let base = s[..s.len() - q.len()].to_string();
            return Some((base, q.to_string()));
        }
    }

    None
                }
