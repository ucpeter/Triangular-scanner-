use crate::models::PairPrice;
use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{info, warn, error};

/// WS-only snapshot collector with heuristic filtering (no REST)
/// - requires a symbol to be seen `MIN_SEEN` times within the window before accepting
/// - restricts accepted quote currencies to a small allowed set
/// - sanity-checks prices (finite, > MIN_PRICE, < MAX_PRICE)
const MIN_SEEN: usize = 2;
const MIN_PRICE: f64 = 1e-12;
const MAX_PRICE: f64 = 1e9;
fn allowed_quotes() -> &'static [&'static str] {
    &["USDT", "BUSD", "USDC", "BTC", "ETH", "BNB"]
}

pub async fn collect_exchange_snapshot(exchange: &str, seconds: u64) -> Vec<PairPrice> {
    let url = match exchange {
        "binance" => "wss://stream.binance.com:9443/ws/!ticker@arr",
        "bybit"   => "wss://stream.bybit.com/v5/public/spot",
        "gateio"  => "wss://api.gateio.ws/ws/v4/",
        "kucoin"  => "wss://ws-api-spot.kucoin.com/?token=public",
        _ => {
            warn!("Unknown exchange {}, defaulting to Binance", exchange);
            "wss://stream.binance.com:9443/ws/!ticker@arr"
        }
    };

    info!("Connecting to {} at {}", exchange, url);

    // Accumulators
    let mut seen: HashMap<String, usize> = HashMap::new();        // symbol -> times seen
    let mut last_price: HashMap<String, f64> = HashMap::new();    // symbol -> last price parsed
    let mut accepted: HashSet<String> = HashSet::new();           // accepted symbols (unique)
    let mut skipped: usize = 0;

    // We'll return a Vec of PairPrice for accepted symbols (unique)
    let mut pairs: Vec<PairPrice> = Vec::new();

    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            // Send exchange-specific subscribe messages (WS-only)
            if exchange == "bybit" {
                let sub = serde_json::json!({ "op": "subscribe", "args": ["tickers"] });
                let _ = ws_stream.send(Message::Text(sub.to_string())).await;
            } else if exchange == "gateio" {
                // Gate: spot.tickers channel
                let sub = serde_json::json!({
                    "time": chrono::Utc::now().timestamp_millis(),
                    "channel":"spot.tickers",
                    "event":"subscribe",
                    "payload":[]
                });
                let _ = ws_stream.send(Message::Text(sub.to_string())).await;
            } else if exchange == "kucoin" {
                let sub = serde_json::json!({
                    "id":"scanner",
                    "type":"subscribe",
                    "topic":"/market/ticker:all",
                    "response": true
                });
                let _ = ws_stream.send(Message::Text(sub.to_string())).await;
            }

            let deadline = Instant::now() + Duration::from_secs(seconds);
            let allowed = allowed_quotes().iter().copied().collect::<HashSet<&str>>();

            // Read messages until deadline
            while Instant::now() < deadline {
                tokio::select! {
                    msg = ws_stream.next() => {
                        match msg {
                            Some(Ok(m)) if m.is_text() => {
                                if let Ok(txt) = m.into_text() {
                                    if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                                        // parse depending on exchange
                                        let maybe_ticker = match exchange {
                                            "binance" => parse_binance_ticker(&v),
                                            "bybit" => parse_bybit_ticker(&v),
                                            "gateio" => parse_gateio_ticker(&v),
                                            "kucoin" => parse_kucoin_ticker(&v),
                                            _ => None,
                                        };

                                        if let Some((sym, price)) = maybe_ticker {
                                            // basic sanity checks
                                            if !price.is_finite() || price <= MIN_PRICE || price >= MAX_PRICE {
                                                skipped += 1;
                                                continue;
                                            }

                                            // normalize symbol and split
                                            let s = sym.to_uppercase();
                                            let (base, quote) = split_symbol(&s);
                                            if base.is_empty() || quote.is_empty() {
                                                skipped += 1;
                                                continue;
                                            }

                                            // allow only selected quote currencies (reduces fiat/test noise)
                                            if !allowed.contains(quote.as_str()) {
                                                skipped += 1;
                                                continue;
                                            }

                                            // increment seen count
                                            let cnt = seen.entry(s.clone()).and_modify(|c| *c += 1).or_insert(1);
                                            last_price.insert(s.clone(), price);

                                            // accept only after seen >= MIN_SEEN and not already accepted
                                            if *cnt >= MIN_SEEN && !accepted.contains(&s) {
                                                // add unique PairPrice
                                                pairs.push(PairPrice {
                                                    base: base.clone(),
                                                    quote: quote.clone(),
                                                    price,
                                                    is_spot: true,
                                                });
                                                accepted.insert(s.clone());
                                            }
                                        }
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                error!("{} ws read error: {:?}", exchange, e);
                                break;
                            }
                            None => break,
                            _ => {}
                        }
                    }
                    // also break if deadline reached
                    _ = tokio::time::sleep_until(deadline) => break,
                }
            } // end while
        }
        Err(e) => {
            error!("{} connect error: {:?}", exchange, e);
        }
    } // end match connect

    info!("{} collected {} unique pairs, seen_total={}, skipped={}",
           exchange, pairs.len(), seen.len(), skipped);

    pairs
}

/// Parse helpers return Option<(symbol, price)> depending on exchange message format
fn parse_binance_ticker(v: &Value) -> Option<(String, f64)> {
    // Binance feed is array-of-objects for !ticker@arr; handle both array and single
    if let Value::Array(arr) = v {
        // Not expected here since caller parses outer array iteratively; but keep safe
        return None;
    }
    // Some Binance messages might be single object
    if let Some(sym) = v.get("s").and_then(|x| x.as_str()) {
        // prefer "c" (last price) that is sometimes string
        let price = v.get("c").and_then(|x| x.as_str()).and_then(|s| s.parse::<f64>().ok())
                .or_else(|| v.get("c").and_then(|x| x.as_f64()));
        if let Some(p) = price { return Some((sym.to_string(), p)); }
    }
    None
}

fn parse_bybit_ticker(v: &Value) -> Option<(String, f64)> {
    // Bybit tickers come under {"topic":"tickers","data":[...]}
    if v.get("topic").and_then(|t| t.as_str()) == Some("tickers") {
        if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
            // we return the first item — caller processes many messages anyway
            if let Some(it) = arr.get(0) {
                if let (Some(sym), Some(price_str)) = (it.get("symbol").and_then(|s| s.as_str()),
                                                      it.get("lastPrice").and_then(|p| p.as_str())) {
                    if let Ok(p) = price_str.parse::<f64>() { return Some((sym.to_string(), p)); }
                }
            }
        }
    }
    None
}

fn parse_gateio_ticker(v: &Value) -> Option<(String, f64)> {
    // Gate: messages use channel "spot.tickers" with result array
    if v.get("channel").and_then(|c| c.as_str()) == Some("spot.tickers") {
        if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
            if let Some(it) = arr.get(0) {
                if let (Some(sym), Some(last)) = (it.get("currency_pair").and_then(|s| s.as_str()),
                                                  it.get("last").and_then(|s| s.as_f64())) {
                    return Some((sym.to_string(), last));
                }
            }
        }
    }
    None
}

fn parse_kucoin_ticker(v: &Value) -> Option<(String, f64)> {
    // Kucoin often sends { "data": { "symbol": "...", "price":"..." } }
    if let Some(data) = v.get("data") {
        if let (Some(sym), Some(price_str)) = (data.get("symbol").and_then(|s| s.as_str()),
                                              data.get("price").and_then(|p| p.as_str())) {
            if let Ok(p) = price_str.parse::<f64>() { return Some((sym.to_string(), p)); }
        }
    }
    None
}

/// Split symbols like "BTCUSDT", "ETH-BTC", "BTC_USDT"
fn split_symbol(sym: &str) -> (String, String) {
    let s = sym.to_uppercase();

    // common separators
    if s.contains('-') {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 2 { return (parts[0].to_string(), parts[1].to_string()); }
    }
    if s.contains('_') {
        let parts: Vec<&str> = s.split('_').collect();
        if parts.len() == 2 { return (parts[0].to_string(), parts[1].to_string()); }
    }

    // no separator: detect known quote suffix
    let suffixes = allowed_quotes();
    for suf in suffixes {
        if s.ends_with(suf) && s.len() > suf.len() {
            let base = s[..s.len() - suf.len()].to_string();
            return (base, suf.to_string());
        }
    }

    // fallback: last 3 or 4 chars as quote (risky)
    if s.len() > 3 {
        let (a,b) = s.split_at(s.len()-3);
        return (a.to_string(), b.to_string());
    }

    (String::new(), String::new())
                  }
