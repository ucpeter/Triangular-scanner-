use crate::models::PairPrice;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TriangularResult {
    pub triangle: (String, String, String),
    pub pairs: (PairPrice, PairPrice, PairPrice),
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
    pub liquidity_score: f64,
}

/// Static taker fees (as fractions, e.g. 0.001 = 0.1%)
fn exchange_fee(exchange: &str) -> f64 {
    match exchange {
        "binance" => 0.001,
        "bybit"   => 0.001,
        "gateio"  => 0.002,
        "kucoin"  => 0.001,
        _ => 0.001,
    }
}

/// Filter out unrealistic or illiquid pairs.
fn filter_liquid_pairs(pairs: Vec<PairPrice>) -> Vec<PairPrice> {
    pairs
        .into_iter()
        .filter(|p| p.price > 0.0 && p.price.is_finite())
        .collect()
}

/// Very simple liquidity scoring function.
fn liquidity_score(p1: &PairPrice, p2: &PairPrice, p3: &PairPrice) -> f64 {
    // Right now just averages the inverse price (acts as proxy for affordability/liquidity).
    // Later this could be extended with REST-based volume data.
    let avg_price = (p1.price + p2.price + p3.price) / 3.0;
    if avg_price > 0.0 {
        1.0 / avg_price
    } else {
        0.0
    }
}

/// Find triangular arbitrage opportunities with liquidity filter.
pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let mut out: Vec<TriangularResult> = Vec::new();
    let fee_rate = exchange_fee(exchange);

    let clean_pairs = filter_liquid_pairs(pairs);
    let mut map: HashMap<(String, String), f64> = HashMap::new();
    for p in &clean_pairs {
        map.insert((p.base.clone(), p.quote.clone()), p.price);
    }

    let symbols: Vec<String> = clean_pairs.iter().map(|p| p.base.clone()).collect();

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

                    let profit_before = (after3 - start) / start * 100.0;

                    if profit_before > min_profit {
                        let fees = 3.0 * fee_rate * 100.0;
                        let profit_after = profit_before - fees;

                        if profit_after > min_profit {
                            if let (Some(pp1), Some(pp2), Some(pp3)) = (
                                clean_pairs.iter().find(|p| p.base == *a && p.quote == *b),
                                clean_pairs.iter().find(|p| p.base == *b && p.quote == *c),
                                clean_pairs.iter().find(|p| p.base == *c && p.quote == *a),
                            ) {
                                let liq_score = liquidity_score(pp1, pp2, pp3);
                                out.push(TriangularResult {
                                    triangle: (a.clone(), b.clone(), c.clone()),
                                    pairs: (pp1.clone(), pp2.clone(), pp3.clone()),
                                    profit_before,
                                    fees,
                                    profit_after,
                                    liquidity_score: liq_score,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort primarily by profit_after, secondarily by liquidity_score
    out.sort_by(|x, y| {
        y.profit_after
            .partial_cmp(&x.profit_after)
            .unwrap()
            .then(y.liquidity_score.partial_cmp(&x.liquidity_score).unwrap())
    });

    out
        }
