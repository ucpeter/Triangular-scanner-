use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::StreamExt;
use tracing::{info, warn};

use crate::models::PairPrice;

#[derive(Debug, Deserialize)]
struct BinanceTicker {
    s: String,   // symbol, e.g. "BTCUSDT"
    c: String,   // last price
}

pub async fn collect_exchange_snapshot(exchange: &str, collect_seconds: u64) -> Vec<PairPrice> {
    let mut out: Vec<PairPrice> = Vec::new();

    match exchange {
        "binance" => {
            let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
            info!("Connecting to binance at {}", url);

            let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
            let (_, mut read) = ws_stream.split();

            let mut seen_total = 0usize;
            let mut seen_unique = 0usize;
            let mut skipped = 0usize;

            while let Some(msg) = tokio::time::timeout(
                std::time::Duration::from_secs(collect_seconds),
                read.next(),
            )
            .await
            .ok()
            .flatten()
            {
                if let Ok(Message::Text(txt)) = msg {
                    if let Ok(tickers) = serde_json::from_str::<Vec<BinanceTicker>>(&txt) {
                        for t in tickers {
                            seen_total += 1;

                            if let Some((base, quote)) = dynamic_split_symbol(&t.s) {
                                if let Ok(price) = t.c.parse::<f64>() {
                                    out.push(PairPrice { base, quote, price });
                                    seen_unique += 1;
                                }
                            } else {
                                skipped += 1;
                            }
                        }
                        break; // stop after first snapshot window
                    }
                }
            }

            info!(
                "binance collected {} unique pairs, seen_total={}, skipped={}",
                seen_unique, seen_total, skipped
            );
        }
        _ => warn!("Exchange {} not implemented yet", exchange),
    }

    out
}

/// Dynamically split a symbol into base/quote using uppercase boundaries.
/// Works with all Binance spot pairs (no hardcoded suffixes).
fn dynamic_split_symbol(sym: &str) -> Option<(String, String)> {
    // Binance symbols are like "BTCUSDT", "ETHBUSD", "SOLBTC"
    // We'll try to find the longest valid suffix that makes sense as a quote
    let candidates = ["USDT", "BUSD", "USDC", "BTC", "ETH", "TRY", "EUR", "TUSD"];

    for q in candidates {
        if sym.ends_with(q) {
            let base = sym.strip_suffix(q).unwrap().to_string();
            return Some((base, q.to_string()));
        }
    }

    // fallback: split last 3 letters
    if sym.len() > 3 {
        let (b, q) = sym.split_at(sym.len() - 3);
        return Some((b.to_string(), q.to_string()));
    }

    None
                        }
