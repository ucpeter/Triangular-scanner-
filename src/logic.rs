use crate::models::{PairPrice, TriangularResult};
use std::collections::HashMap;

pub fn find_triangular_arbitrage(prices: Vec<PairPrice>) -> Vec<TriangularResult> {
    let mut out = Vec::new();
    let mut map: HashMap<(String,String), f64> = HashMap::new();

    for p in &prices {
        map.insert((p.base.clone(), p.quote.clone()), p.price);
    }

    for a in ["USDT","BTC","ETH","BUSD"] {
        for b in ["USDT","BTC","ETH","BUSD"] {
            for c in ["USDT","BTC","ETH","BUSD"] {
                if a == b || b == c || a == c { continue; }
                if let (Some(p1), Some(p2), Some(p3)) = (
                    map.get(&(a.to_string(), b.to_string())),
                    map.get(&(b.to_string(), c.to_string())),
                    map.get(&(c.to_string(), a.to_string())),
                ) {
                    let start = 100.0;
                    let result = start / p1 * p2 * p3;
                    if result > start {
                        out.push(TriangularResult {
                            route: format!("{} -> {} -> {} -> {}", a, b, c, a),
                            profit_before: result - start,
                            profit_after: result - start,
                        });
                    }
                }
            }
        }
    }

    out
                        }
