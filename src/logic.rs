use crate::models::{PairPrice, TriangularResult};
use std::collections::{HashMap, HashSet};

/// Find triangular opportunities from a list of spot pair prices.
/// - `pairs`: list of PairPrice (base, quote, price)
/// - `min_profit`: minimum profit BEFORE fees (percent)
pub fn find_triangular_opportunities(pairs: &Vec<PairPrice>, min_profit: f64) -> Vec<TriangularResult> {
    // Build rate map and neighbor adjacency (only spot and valid prices)
    let mut rate: HashMap<(String, String), f64> = HashMap::new();
    let mut neighbors: HashMap<String, HashSet<String>> = HashMap::new();

    for p in pairs {
        if !p.is_spot { continue; }
        if !p.price.is_finite() || p.price <= 0.0 { continue; }

        let a = p.base.to_uppercase();
        let b = p.quote.to_uppercase();

        rate.insert((a.clone(), b.clone()), p.price);
        neighbors.entry(a.clone()).or_default().insert(b.clone());

        // add inverse edge
        if p.price > 0.0 {
            rate.insert((b.clone(), a.clone()), 1.0 / p.price);
            neighbors.entry(b.clone()).or_default().insert(a.clone());
        }
    }

    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    let mut out: Vec<TriangularResult> = Vec::new();

    // fee per leg (percent). Adjust if you calculate fees elsewhere.
    let fee_per_leg = 0.10_f64; // 0.10% per trade as default
    let fee_factor = 1.0 - (fee_per_leg / 100.0);
    let total_fee_factor = fee_factor * fee_factor * fee_factor;
    // total fee percent (for reporting if needed) = 3 * fee_per_leg

    // Iterate over triples A -> B -> C -> A
    for a in neighbors.keys() {
        let a = a.clone();
        if let Some(bs) = neighbors.get(&a) {
            for b in bs {
                if a == *b { continue; }
                if let Some(cs) = neighbors.get(b) {
                    for c in cs {
                        if c == &a || c == b { continue; }
                        // ensure c -> a exists
                        if !neighbors.get(c).map_or(false, |set| set.contains(&a)) { continue; }

                        // fetch rates safely
                        let r1 = match rate.get(&(a.clone(), b.clone())) { Some(v) => *v, None => continue };
                        let r2 = match rate.get(&(b.clone(), c.clone())) { Some(v) => *v, None => continue };
                        let r3 = match rate.get(&(c.clone(), a.clone())) { Some(v) => *v, None => continue };

                        if !r1.is_finite() || !r2.is_finite() || !r3.is_finite() { continue; }
                        if r1 <= 0.0 || r2 <= 0.0 || r3 <= 0.0 { continue; }

                        let cycle = r1 * r2 * r3;
                        let profit_before = (cycle - 1.0) * 100.0;

                        // sanity checks
                        if !profit_before.is_finite() { continue; }
                        if profit_before < min_profit { continue; }

                        let profit_after = (cycle * total_fee_factor - 1.0) * 100.0;
                        if !profit_after.is_finite() { continue; }

                        // canonical dedupe (rotations)
                        let reps = vec![
                            (a.clone(), b.clone(), c.clone()),
                            (b.clone(), c.clone(), a.clone()),
                            (c.clone(), a.clone(), b.clone()),
                        ];
                        let key = reps.iter().min().unwrap().clone();
                        if !seen.insert(key) { continue; }

                        // round to 2 decimal places for nicer output
                        let profit_before_rounded = (profit_before * 100.0).round() / 100.0;
                        let profit_after_rounded  = (profit_after  * 100.0).round() / 100.0;

                        out.push(TriangularResult {
                            route: format!("{} → {} → {} → {}", a, b, c, a),
                            profit_before: profit_before_rounded,
                            profit_after: profit_after_rounded,
                        });
                    }
                }
            }
        }
    }

    // sort by profit_after desc
    out.sort_by(|x, y| y.profit_after.partial_cmp(&x.profit_after).unwrap_or(std::cmp::Ordering::Equal));
    out
                }
