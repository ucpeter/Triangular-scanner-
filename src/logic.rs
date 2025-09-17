use crate::models::PairPrice;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TriangularResult {
    pub triangle: (String, String, String),
    pub pairs: (PairPrice, PairPrice, PairPrice),
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
}

/// Static taker fees (as fractions)
fn exchange_fee(exchange: &str) -> f64 {
    match exchange {
        "binance" => 0.001,
        "bybit"   => 0.001,
        "gateio"  => 0.002,
        "kucoin"  => 0.001,
        _ => 0.001,
    }
}

/// High liquidity quotes
fn is_high_liquidity(quote: &str) -> bool {
    matches!(quote, "USDT" | "USDC" | "BTC" | "ETH" | "BNB")
}

/// Main arbitrage finder with liquidity priority
pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    // Split pairs into high-liquidity vs others
    let mut high_liq: Vec<PairPrice> = Vec::new();
    let mut other: Vec<PairPrice> = Vec::new();
    for p in pairs {
        if is_high_liquidity(&p.quote) {
            high_liq.push(p);
        } else {
            other.push(p);
        }
    }

    let fee_rate = exchange_fee(exchange);

    // First scan high-liquidity pairs
    let mut results = run_scan(&high_liq, min_profit, fee_rate);

    if results.is_empty() {
        // If nothing found, scan all pairs
        let mut all = high_liq;
        all.extend(other);
        results = run_scan(&all, min_profit, fee_rate);
    }

    results.sort_by(|x, y| y.profit_after.partial_cmp(&x.profit_after).unwrap());
    results
}

fn run_scan(
    pairs: &Vec<PairPrice>,
    min_profit: f64,
    fee_rate: f64,
) -> Vec<TriangularResult> {
    let mut out: Vec<TriangularResult> = Vec::new();

    // Build lookup
    let mut map: HashMap<(String, String), f64> = HashMap::new();
    for p in pairs {
        map.insert((p.base.clone(), p.quote.clone()), p.price);
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

    out
        }
