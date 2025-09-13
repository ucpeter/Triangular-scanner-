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

/// Static taker fees (as fractions, e.g. 0.001 = 0.1%)
fn exchange_fee(exchange: &str) -> f64 {
    match exchange {
        "binance" => 0.001, // 0.1%
        "bybit"   => 0.001, // 0.1%
        "gateio"  => 0.002, // 0.2%
        "kucoin"  => 0.001, // 0.1%
        _ => 0.001,         // default 0.1%
    }
}

/// Very simple triangular arbitrage finder
pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let mut out: Vec<TriangularResult> = Vec::new();
    let fee_rate = exchange_fee(exchange);

    // Build lookup: base+quote -> price
    let mut map: HashMap<(String, String), f64> = HashMap::new();
    for p in &pairs {
        map.insert((p.base.clone(), p.quote.clone()), p.price);
    }

    // Collect symbol list (unique bases and quotes)
    let mut symbols: Vec<String> = Vec::new();
    for p in &pairs {
        if !symbols.contains(&p.base) {
            symbols.push(p.base.clone());
        }
        if !symbols.contains(&p.quote) {
            symbols.push(p.quote.clone());
        }
    }

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
                    // Start with 1 unit of A
                    // A -> B using A/B price: amount_B = amount_A / price(A/B)
                    let start = 1.0;
                    let after1 = start / p1;
                    let after2 = after1 / p2;
                    let after3 = after2 * p3;

                    let profit_before = (after3 - start) / start * 100.0;

                    if profit_before > min_profit {
                        // fees expressed as percentage points (3 trades)
                        let fees = 3.0 * fee_rate * 100.0;
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

    out.sort_by(|x, y| y.profit_after.partial_cmp(&x.profit_after).unwrap_or(std::cmp::Ordering::Equal));
    out
        }
