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

/// Static taker fees (fractions, e.g. 0.001 = 0.1%)
fn exchange_fee(exchange: &str) -> f64 {
    match exchange {
        "binance" => 0.001,
        "bybit"   => 0.001,
        "gateio"  => 0.002,
        "kucoin"  => 0.001,
        _ => 0.001,
    }
}

/// Triangular arbitrage finder
pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let mut out: Vec<TriangularResult> = Vec::new();
    let fee_rate = exchange_fee(exchange);

    // Build lookup: (base, quote) -> price
    let mut map: HashMap<(String, String), f64> = HashMap::new();
    for p in &pairs {
        map.insert((p.base.clone(), p.quote.clone()), p.price);
    }

    let symbols: Vec<String> = pairs.iter().map(|p| p.base.clone()).collect();

    for a in &symbols {
        for b in &symbols {
            if a == b { continue; }
            for c in &symbols {
                if c == a || c == b { continue; }

                // Test both directions: A→B→C→A and A→C→B→A
                let directions = vec![
                    ((a.clone(), b.clone()), (b.clone(), c.clone()), (c.clone(), a.clone())),
                    ((a.clone(), c.clone()), (c.clone(), b.clone()), (b.clone(), a.clone())),
                ];

                for (p1, p2, p3) in directions {
                    if let (Some(&r1), Some(&r2), Some(&r3)) =
                        (map.get(&(p1.0.clone(), p1.1.clone())),
                         map.get(&(p2.0.clone(), p2.1.clone())),
                         map.get(&(p3.0.clone(), p3.1.clone())))
                    {
                        // Start with 1 unit
                        let mut amount = 1.0;
                        amount = amount / r1; // first trade
                        amount = amount / r2; // second trade
                        amount = amount * r3; // third trade

                        let profit_before = (amount - 1.0) * 100.0;

                        // Apply fee per leg (multiplicative)
                        let after_fees = amount * (1.0 - fee_rate).powi(3);
                        let profit_after = (after_fees - 1.0) * 100.0;

                        if profit_after > min_profit {
                            if let (Some(pp1), Some(pp2), Some(pp3)) = (
                                pairs.iter().find(|p| p.base == p1.0 && p.quote == p1.1),
                                pairs.iter().find(|p| p.base == p2.0 && p.quote == p2.1),
                                pairs.iter().find(|p| p.base == p3.0 && p.quote == p3.1),
                            ) {
                                out.push(TriangularResult {
                                    triangle: (p1.0.clone(), p2.0.clone(), p3.0.clone()),
                                    pairs: (pp1.clone(), pp2.clone(), pp3.clone()),
                                    profit_before,
                                    fees: fee_rate * 100.0 * 3.0,
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
