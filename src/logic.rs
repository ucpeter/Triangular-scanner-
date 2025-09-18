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
}

/// Static taker fees (fraction, e.g. 0.001 = 0.1%)
fn exchange_fee(exchange: &str) -> f64 {
    match exchange {
        "binance" => 0.001, // 0.1%
        "bybit"   => 0.001,
        "gateio"  => 0.002,
        "kucoin"  => 0.001,
        _ => 0.001,
    }
}

/// Filter out pairs with poor liquidity (tiny or illiquid)
fn is_liquid(price: f64) -> bool {
    // crude rule: ignore "dust" pairs
    price > 0.0000001 && price.is_finite()
}

/// Find triangular arbitrage opportunities (with liquidity check & speed optimization).
pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let fee_rate = exchange_fee(exchange);

    // Build quick lookup: (base, quote) -> price
    let mut map: HashMap<(String, String), f64> = HashMap::new();
    for p in &pairs {
        if is_liquid(p.price) {
            map.insert((p.base.clone(), p.quote.clone()), p.price);
        }
    }

    let symbols: Vec<String> = pairs.iter().map(|p| p.base.clone()).collect();

    let mut out: Vec<TriangularResult> = Vec::new();

    for a in &symbols {
        for b in &symbols {
            if a == b { continue; }
            for c in &symbols {
                if c == a || c == b { continue; }

                // Path: A/B -> B/C -> C/A
                if let (Some(p1), Some(p2), Some(p3)) = (
                    map.get(&(a.clone(), b.clone())),
                    map.get(&(b.clone(), c.clone())),
                    map.get(&(c.clone(), a.clone())),
                ) {
                    // simulate trades
                    let start = 1.0;
                    let after1 = start / p1;
                    let after2 = after1 / p2;
                    let after3 = after2 * p3;

                    let profit_before = (after3 - start) / start * 100.0;

                    if profit_before > min_profit {
                        let fees = 3.0 * fee_rate * 100.0; // 3 trades
                        let profit_after = profit_before - fees;

                        if profit_after > min_profit {
                            if let (Some(pp1), Some(pp2), Some(pp3)) = (
                                pairs.iter().find(|p| p.base == *a && p.quote == *b),
                                pairs.iter().find(|p| p.base == *b && p.quote == *c),
                                pairs.iter().find(|p| p.base == *c && p.quote == *a),
                            ) {
                                out.push(TriangularResult {
                                    triangle: (a.clone(), b.clone(), c.clone()),
                                    pairs: (pp1.clone(), pp2.clone(), pp3.clone()),
                                    profit_before,
                                    fees,
                                    profit_after,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // sort best opportunities on top
    out.sort_by(|x, y| y.profit_after.partial_cmp(&x.profit_after).unwrap());
    out
        }
