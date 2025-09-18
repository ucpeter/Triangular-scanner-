use crate::models::PairPrice;
use std::collections::HashMap;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TriangularResult {
    pub triangle: (String, String, String),
    pub pairs: (PairPrice, PairPrice, PairPrice),
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
    pub liquidity_score: f64,
}

/// Static taker fees (fraction: 0.001 = 0.1%)
fn exchange_fee(exchange: &str) -> f64 {
    match exchange {
        "binance" => 0.001,
        "bybit"   => 0.001,
        "gateio"  => 0.002,
        "kucoin"  => 0.001,
        _ => 0.001,
    }
}

/// Very simple liquidity function.
/// For now: price * min(volume_base, volume_quote) if available.
fn estimate_liquidity(pair: &PairPrice) -> f64 {
    // If PairPrice has no volume, return a fallback value (acts as low liquidity)
    if let Some(vol) = pair.volume {
        pair.price * vol
    } else {
        pair.price * 1.0
    }
}

/// Triangular arbitrage finder with liquidity prioritisation
pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let mut out: Vec<TriangularResult> = Vec::new();
    let fee_rate = exchange_fee(exchange);

    // Build lookup map
    let mut map: HashMap<(String, String), &PairPrice> = HashMap::new();
    for p in &pairs {
        map.insert((p.base.clone(), p.quote.clone()), p);
    }

    let symbols: Vec<String> = pairs.iter().map(|p| p.base.clone()).collect();

    for a in &symbols {
        for b in &symbols {
            if a == b { continue; }
            for c in &symbols {
                if c == a || c == b { continue; }

                if let (Some(p1), Some(p2), Some(p3)) = (
                    map.get(&(a.clone(), b.clone())),
                    map.get(&(b.clone(), c.clone())),
                    map.get(&(c.clone(), a.clone())),
                ) {
                    // simulate trade A -> B -> C -> A
                    let start = 1.0;
                    let after1 = start / p1.price;
                    let after2 = after1 / p2.price;
                    let after3 = after2 * p3.price;

                    let profit_before = (after3 - start) / start * 100.0;

                    if profit_before > min_profit {
                        let fees = 3.0 * fee_rate * 100.0;
                        let profit_after = profit_before - fees;

                        if profit_after > min_profit {
                            let liquidity_score = [
                                estimate_liquidity(p1),
                                estimate_liquidity(p2),
                                estimate_liquidity(p3),
                            ]
                            .iter()
                            .cloned()
                            .fold(f64::INFINITY, f64::min);

                            out.push(TriangularResult {
                                triangle: (a.clone(), b.clone(), c.clone()),
                                pairs: ((*p1).clone(), (*p2).clone(), (*p3).clone()),
                                profit_before,
                                fees,
                                profit_after,
                                liquidity_score,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort opportunities: first by profit_after, then liquidity_score
    out.sort_by(|x, y| {
        y.profit_after
            .partial_cmp(&x.profit_after)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                y.liquidity_score
                    .partial_cmp(&x.liquidity_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    out
    }
