use crate::models::{PairPrice, TriangularResult};
use crate::utils::round2;
use std::collections::{HashMap, HashSet};

/// Triangular scan using spot PairPrice slice.
/// fee_per_leg is percent (0.1 meaning 0.1%).
pub fn scan_triangles(prices: &[PairPrice], min_profit: f64, fee_per_leg: f64) -> Vec<TriangularResult> {
    let mut rate: HashMap<(String, String), f64> = HashMap::new();
    let mut neighbors: HashMap<String, HashSet<String>> = HashMap::new();

    for p in prices {
        if p.price <= 0.0 { continue; }
        let a = p.base.to_uppercase();
        let b = p.quote.to_uppercase();

        rate.insert((a.clone(), b.clone()), p.price);
        neighbors.entry(a.clone()).or_default().insert(b.clone());

        rate.insert((b.clone(), a.clone()), 1.0 / p.price);
        neighbors.entry(b.clone()).or_default().insert(a.clone());
    }

    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    let mut out: Vec<TriangularResult> = Vec::new();
    let fee_mult = 1.0 - (fee_per_leg / 100.0);
    let total_fees = 3.0 * fee_per_leg;

    for (a, bs) in &neighbors {
        for b in bs {
            if a == b { continue; }
            if let Some(cs) = neighbors.get(b) {
                for c in cs {
                    if c == a || c == b { continue; }
                    if !neighbors.get(c).map_or(false, |s| s.contains(a)) { continue; }

                    let r1 = *rate.get(&(a.clone(), b.clone())).unwrap_or(&0.0);
                    let r2 = *rate.get(&(b.clone(), c.clone())).unwrap_or(&0.0);
                    let r3 = *rate.get(&(c.clone(), a.clone())).unwrap_or(&0.0);
                    if r1 <= 0.0 || r2 <= 0.0 || r3 <= 0.0 { continue; }

                    let gross = r1 * r2 * r3;
                    let profit_before = (gross - 1.0) * 100.0;
                    if !profit_before.is_finite() || profit_before < min_profit { continue; }

                    let net = (r1*fee_mult) * (r2*fee_mult) * (r3*fee_mult);
                    let profit_after = (net - 1.0) * 100.0;

                    // canonical dedupe
                    let reps = vec![
                        (a.clone(), b.clone(), c.clone()),
                        (b.clone(), c.clone(), a.clone()),
                        (c.clone(), a.clone(), b.clone()),
                    ];
                    let key = reps.iter().min().unwrap().clone();
                    if !seen.insert(key) { continue; }

                    out.push(TriangularResult {
                        triangle: format!("{} → {} → {} → {}", a, b, c, a),
                        pairs: format!("{}/{} | {}/{} | {}/{}", a, b, b, c, c, a),
                        profit_before: round2(profit_before),
                        fees: round2(total_fees),
                        profit_after: round2(profit_after),
                    });
                }
            }
        }
    }

    out.sort_by(|x,y| y.profit_after.partial_cmp(&x.profit_after).unwrap_or(std::cmp::Ordering::Equal));
    out
                        }
