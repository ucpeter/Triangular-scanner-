use crate::models::PairPrice;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub triangle: (String, String, String),
    pub pairs: [PairPrice; 3],
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
}

/// Static taker fees per exchange
fn exchange_fee(exchange: &str) -> f64 {
    match exchange {
        "binance" => 0.001,
        "bybit"   => 0.001,
        "gateio"  => 0.002,
        "kucoin"  => 0.001,
        _ => 0.001,
    }
}

/// Get price, trying both directions (A/B or B/A inverted)
fn lookup(map: &HashMap<(String, String), f64>, a: &str, b: &str) -> Option<(f64, bool)> {
    if let Some(p) = map.get(&(a.to_string(), b.to_string())) {
        Some((*p, false)) // normal A/B
    } else if let Some(p) = map.get(&(b.to_string(), a.to_string())) {
        Some((1.0 / *p, true)) // inverted B/A
    } else {
        None
    }
}

pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let mut out: Vec<TriangularResult> = Vec::new();
    let fee_rate = exchange_fee(exchange);

    let mut map: HashMap<(String, String), f64> = HashMap::new();
    for p in &pairs {
        map.insert((p.base.clone(), p.quote.clone()), p.price);
    }

    let mut symbols: Vec<String> = Vec::new();
    for p in &pairs {
        if !symbols.contains(&p.base) { symbols.push(p.base.clone()); }
        if !symbols.contains(&p.quote) { symbols.push(p.quote.clone()); }
    }

    for a in &symbols {
        for b in &symbols {
            if a == b { continue; }
            for c in &symbols {
                if c == a || c == b { continue; }

                if let (Some((p1, _inv1)), Some((p2, _inv2)), Some((p3, _inv3))) = (
                    lookup(&map, a, b),
                    lookup(&map, b, c),
                    lookup(&map, c, a),
                ) {
                    let start = 1.0;
                    let after1 = start / p1;
                    let after2 = after1 / p2;
                    let after3 = after2 * p3;

                    let profit_before = (after3 - start) * 100.0;
                    if profit_before > min_profit {
                        let fees = 3.0 * fee_rate * 100.0;
                        let profit_after = profit_before - fees;

                        if profit_after > min_profit {
                            if let (Some(pp1), Some(pp2), Some(pp3)) = (
                                pairs.iter().find(|p| 
                                    (p.base == *a && p.quote == *b) || (p.base == *b && p.quote == *a)),
                                pairs.iter().find(|p| 
                                    (p.base == *b && p.quote == *c) || (p.base == *c && p.quote == *b)),
                                pairs.iter().find(|p| 
                                    (p.base == *c && p.quote == *a) || (p.base == *a && p.quote == *c)),
                            ) {
                                out.push(TriangularResult {
                                    triangle: (a.clone(), b.clone(), c.clone()),
                                    pairs: [pp1.clone(), pp2.clone(), pp3.clone()],
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

    out.sort_by(|x, y| y.profit_after.partial_cmp(&x.profit_after).unwrap());
    out
        }
