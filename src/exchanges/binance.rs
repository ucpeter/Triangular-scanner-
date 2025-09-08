use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use crate::models::PairPrice;
use tracing::{info, warn};

use crate::ws_manager::SharedPrices;

/// Binance "pro" aggregated all tickers stream: wss://stream.binance.com:9443/ws/!ticker@arr
pub async fn run_binance_ws(prices: SharedPrices) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "wss://stream.binance.com:9443/ws/!ticker@arr";
    info!("connecting to Binance WS {}", url);
    let (ws_stream, _) = connect_async(url).await?;
    let (_write, mut read) = ws_stream.split();

    // We'll keep a local map symbol->PairPrice and periodically write to shared state
    let mut local_map: HashMap<String, PairPrice> = HashMap::new();

    while let Some(msg) = read.next().await {
        let msg = msg?;
        if msg.is_text() {
            let text = msg.into_text()?;
            // Binance sends an array of tickers (each object has s = symbol, c = close price)
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                if let Some(arr) = json.as_array() {
                    for it in arr {
                        // Typical fields: s (symbol), c (lastPrice), v (volume)
                        let symbol = it.get("s").and_then(|v| v.as_str()).unwrap_or("");
                        let price_s = it.get("c").and_then(|v| v.as_str()).or_else(|| it.get("c").and_then(|v| v.as_f64().map(|p| p.to_string().as_str()))) ;
                        // price may be string; handle gracefully
                        let price_opt = it.get("c")
                            .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                            .or_else(|| it.get("c").and_then(|v| v.as_f64()));
                        if !symbol.is_empty() {
                            if let Some(price) = price_opt {
                                // Split symbol into base/quote by looking for common quote suffixes
                                let (base, quote) = split_symbol_binance(symbol);
                                if !base.is_empty() && !quote.is_empty() && price > 0.0 {
                                    local_map.insert(symbol.to_string(), PairPrice { base, quote, price });
                                }
                            }
                        }
                    }

                    // flush local_map to shared prices for "binance"
                    let mut guard = prices.write().await;
                    let vec: Vec<PairPrice> = local_map.values().cloned().collect();
                    guard.insert("binance".to_string(), vec);
                }
            } else {
                warn!("binance: failed to parse msg as json (not fatal)");
            }
        }
    }

    Ok(())
}

fn split_symbol_binance(sym: &str) -> (String, String) {
    // common quote suffixes on Binance
    let suffixes = ["USDT", "USDC", "BUSD", "BTC", "ETH", "BNB"];
    for s in &suffixes {
        if sym.ends_with(s) && sym.len() > s.len() {
            let base = sym.trim_end_matches(s).to_string();
            return (base, s.to_string());
        }
    }
    // fallback: try to split by last 3 letters
    if sym.len() > 3 {
        let (a,b) = sym.split_at(sym.len()-3);
        return (a.to_string(), b.to_string());
    }
    (String::new(), String::new())
      }
