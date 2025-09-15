use crate::models::PairPrice;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, serde::Serialize)]
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
        _ => 0.001,
    }
}

/// Build a graph: base -> (quote -> price)
fn build_graph(pairs: &[PairPrice]) -> HashMap<String, HashMap<String, f64>> {
    let mut graph: HashMap<String, HashMap<String, f64>> = HashMap::new();

    for p in pairs {
        graph
            .entry(p.base.clone())
            .or_default()
            .insert(p.quote.clone(), p.price);

        // also store inverse (quote/base) if the pair is invertible
        if p.price > 0.0 {
            graph
                .entry(p.quote.clone())
                .or_default()
                .insert(p.base.clone(), 1.0 / p.price);
        }
    }

    graph
}

/// Very simple triangular arbitrage finder using graph traversal
pub fn find_triangular_opportunities(
    exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let mut out: Vec<TriangularResult> = Vec::new();
    let fee_rate = exchange_fee(exchange);

    let graph = build_graph(&pairs);

    let mut seen: HashSet<(String, String, String)> = HashSet::new();

    for a in graph.keys() {
        for (b, p1) in graph.get(a).unwrap() {
            for (c, p2) in graph.get(b).unwrap_or(&HashMap::new()) {
                if c == a || c == b {
                    continue;
                }
                if let Some(p3) = graph.get(c).and_then(|m| m.get(a)) {
                    // triangle is (a,b,c)
                    let mut triangle = vec![a.clone(), b.clone(), c.clone()];
                    triangle.sort();
                    let key = (triangle[0].clone(), triangle[1].clone(), triangle[2].clone());

                    if seen.contains(&key) {
                        continue;
                    }
                    seen.insert(key);

                    let start = 1.0;
                    let after1 = start * p1;
                    let after2 = after1 * p2;
                    let after3 = after2 * p3;

                    let profit_before = (after3 - start) / start * 100.0;

                    if profit_before > min_profit {
                        let fees = 3.0 * fee_rate * 100.0;
                        let profit_after = profit_before - fees;

                        if profit_after > min_profit {
                            // find actual PairPrice objects
                            let pp1 = pairs.iter().find(|p| {
                                (p.base == *a && p.quote == *b)
                                    || (p.base == *b && p.quote == *a)
                            });
                            let pp2 = pairs.iter().find(|p| {
                                (p.base == *b && p.quote == *c)
                                    || (p.base == *c && p.quote == *b)
                            });
                            let pp3 = pairs.iter().find(|p| {
                                (p.base == *c && p.quote == *a)
                                    || (p.base == *a && p.quote == *c)
                            });

                            if let (Some(pp1), Some(pp2), Some(pp3)) = (pp1, pp2, pp3) {
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

    out.sort_by(|x, y| {
        y.profit_after
            .partial_cmp(&x.profit_after)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    out
        }
