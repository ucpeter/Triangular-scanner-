use crate::models::PairPrice;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async};
use tracing::{info, warn, error};

/// Collect a snapshot of Binance spot tickers using WS only.
pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    if exchange != "binance" {
        warn!("Exchange {} not supported in WS-only mode", exchange);
        return Vec::new();
    }

    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("Connecting to Binance WS at {}", url);

    let mut pairs: Vec<PairPrice> = Vec::new();

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
                                if let Value::Array(arr) = v {
                                    for it in arr {
                                        if let (Some(sym), Some(price)) =
                                            (it.get("s").and_then(|v| v.as_str()),
                                             it.get("c").and_then(|v| v.as_str())
                                                        .and_then(|s| s.parse::<f64>().ok()))
                                        {
                                            // Optional volume field
                                            let vol = it.get("v")
                                                .and_then(|v| v.as_str())
                                                .and_then(|s| s.parse::<f64>().ok());

                                            if price > 0.0 {
                                                if let Some((base, quote)) = split_symbol(sym) {
                                                    pairs.push(PairPrice {
                                                        base,
                                                        quote,
                                                        price,
                                                        is_spot: true,
                                                        volume: vol, // ✅ now matches Option<f64>
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
        }
        Err(e) => {
            error!("binance connect error: {:?}", e);
        }
    }

    info!(
        "scan complete for binance: collected {} pairs",
        pairs.len()
    );

    pairs
}

/// Dynamically splits a Binance symbol into base/quote by trying known suffixes.
fn split_symbol(sym: &str) -> Option<(String, String)> {
    let suffixes = [
        "USDT", "BUSD", "USDC", "FDUSD", "TUSD", "BTC", "ETH", "BNB", "TRY", "EUR", "GBP",
        "AUD", "BRL", "CAD", "ARS", "RUB", "ZAR", "NGN", "UAH", "IDR", "JPY", "KRW", "VND",
        "INR", "MXN", "PLN", "SEK", "CHF",
    ];
    let s = sym.to_uppercase();
    for suf in &suffixes {
        if s.ends_with(suf) && s.len() > suf.len() {
            let base = s[..s.len() - suf.len()].to_string();
            return Some((base, suf.to_string()));
        }
    }
    None
                                        }
