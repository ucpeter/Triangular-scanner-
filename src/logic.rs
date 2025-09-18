use crate::models::PairPrice;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TriangularResult {
    pub triangle: (String, String, String),
    pub pairs: (PairPrice, PairPrice, PairPrice),
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
    pub liquidity_score: f64, // new: total liquidity used
}

/// Static taker fees (fractions)
fn exchange_fee(exchange: &str) -> f64 {
    match exchange {
        "binance" => 0.001,
        "bybit"   => 0.001,
        "gateio"  => 0.002,
        "kucoin"  => 0.001,
        _ => 0.001,
    }
}

/// Finds triangular arbitrage but prioritizes liquidity
pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let mut out: Vec<TriangularResult> = Vec::new();
    let fee_rate = exchange_fee(exchange);

    // filter low-liquidity pairs if available
    let pairs: Vec<PairPrice> = pairs
        .into_iter()
        .filter(|p| p.liquidity.unwrap_or(0.0) > 100_000.0) // only $100k+ 24h quote volume
        .collect();

    // build map
    let mut map: HashMap<(String, String), (f64, f64)> = HashMap::new(); // price + liquidity
    for p in &pairs {
        map.insert((p.base.clone(), p.quote.clone()), (p.price, p.liquidity.unwrap_or(0.0)));
    }

    let symbols: Vec<String> = pairs.iter().map(|p| p.base.clone()).collect();

    for a in &symbols {
        for b in &symbols {
            if a == b { continue; }
            for c in &symbols {
                if c == a || c == b { continue; }

                if let (Some((p1, l1)), Some((p2, l2)), Some((p3, l3))) = (
                    map.get(&(a.clone(), b.clone())),
                    map.get(&(b.clone(), c.clone())),
                    map.get(&(c.clone(), a.clone())),
                ) {
                    let start = 1.0;
                    let after1 = start / p1;
                    let after2 = after1 / p2;
                    let after3 = after2 * p3;

                    let profit_before = (after3 - start) * 100.0;
                    let fees = 3.0 * fee_rate * 100.0;
                    let profit_after = profit_before - fees;

                    let liquidity_score = *l1 + *l2 + *l3;

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
                                liquidity_score,
                            });
                        }
                    }
                }
            }
        }
    }

    // sort by profit + liquidity
    out.sort_by(|x, y| {
        y.profit_after
            .partial_cmp(&x.profit_after)
            .unwrap()
            .then(y.liquidity_score.partial_cmp(&x.liquidity_score).unwrap())
    });

    out
        }
