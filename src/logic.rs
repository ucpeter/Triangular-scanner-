// src/logic.rs
use crate::models::{PairPrice, TriangularResult};
use std::collections::{HashMap, HashSet};

/// find_triangular_opportunities
/// - pairs: snapshot vector of PairPrice (base/quote/price/volume)
/// - min_profit_after: minimum profit % after fees to include
/// - fee_per_leg_pct: percent per trade leg (e.g., 0.1 for 0.1%)
/// - neighbor_limit: per-base, keep only top-N outgoing neighbors by volume (liquidity)
pub fn find_triangular_opportunities(
    _exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit_after: f64,
) -> Vec<TriangularResult> {
    let fee_per_leg_pct = 0.10; // default taker per-leg percent (0.10%); adjust if needed
    let neighbor_limit = 100usize; // tune: how many top neighbors per node to consider

    // Build adjacency: base -> (quote -> price)
    let mut adj: HashMap<String, HashMap<String, f64>> = HashMap::new();
    // Build liquidity map: base -> (quote -> volume)
    let mut vol_map: HashMap<String, HashMap<String, f64>> = HashMap::new();

    for p in pairs.iter() {
        // prefer spot pairs only
        if !p.is_spot || p.price <= 0.0 {
            continue;
        }
        let a = p.base.to_uppercase();
        let b = p.quote.to_uppercase();
        // price a/b meaning: 1 a = price * b (we store as given)
        adj.entry(a.clone()).or_default().insert(b.clone(), p.price);
        // also store inverse for quick lookup (1/b -> a)
        if p.price > 0.0 {
            adj.entry(b.clone()).or_default().insert(a.clone(), 1.0 / p.price);
        }
        vol_map.entry(a.clone()).or_default().insert(b.clone(), p.volume);
        vol_map.entry(b.clone()).or_default().insert(a.clone(), p.volume); // approximate inverse liquidity
    }

    // Build neighbor lists limited by liquidity
    let mut neighbors: HashMap<String, Vec<String>> = HashMap::new();
    for (base, targets) in adj.iter() {
        // sort targets by volume desc (use vol_map)
        let mut vec: Vec<(String, f64)> = targets.iter().map(|(q, &price)| {
            let v = vol_map.get(base).and_then(|m| m.get(q)).copied().unwrap_or(0.0);
            (q.clone(), v)
        }).collect();

        vec.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let list: Vec<String> = vec.into_iter()
            .take(neighbor_limit)
            .map(|(q, _v)| q)
            .collect();
        neighbors.insert(base.clone(), list);
    }

    // Build predecessor sets (nodes that have outgoing edge to key)
    let mut preds: HashMap<String, HashSet<String>> = HashMap::new();
    for (u, m) in adj.iter() {
        for v in m.keys() {
            preds.entry(v.clone()).or_default().insert(u.clone());
        }
    }

    // For dedupe, keep a set of canonical triangle keys
    let mut seen: HashSet<(String,String,String)> = HashSet::new();
    let mut out: Vec<TriangularResult> = Vec::new();

    let fee_factor = (1.0 - fee_per_leg_pct / 100.0).powf(3.0); // multiplicative factor for 3 legs
    let total_fee_pct = 3.0 * fee_per_leg_pct;

    // Iterate nodes; for each A, iterate B in neighbors[A], then consider C in intersection(neighbors[B], preds[A])
    for a in neighbors.keys() {
        let neigh_a = neighbors.get(a).unwrap_or(&Vec::new()).clone();
        for b in neigh_a.iter() {
            // neighbors of B (limited) and preds of A -> intersection are candidates for C
            let nb = neighbors.get(b).unwrap_or(&Vec::new());
            let pred_a = preds.get(a).unwrap_or(&HashSet::new());

            // build a HashSet for fast intersection
            let nb_set: HashSet<&String> = nb.iter().collect();

            for c in nb.iter() {
                if c == a || c == b {
                    continue;
                }
                if !pred_a.contains(c) {
                    continue;
                }

                // Lookup rates r_ab, r_bc, r_ca
                let r_ab = match adj.get(a).and_then(|m| m.get(b)) { Some(&v) => v, None => continue };
                let r_bc = match adj.get(b).and_then(|m| m.get(c)) { Some(&v) => v, None => continue };
                let r_ca = match adj.get(c).and_then(|m| m.get(a)) { Some(&v) => v, None => continue };

                // compute gross cycle multiplier
                let gross = r_ab * r_bc * r_ca;
                if !gross.is_finite() { continue; }
                let profit_before = (gross - 1.0) * 100.0;
                if profit_before <= 0.0 {
                    continue;
                }

                // apply fees multiplicatively across legs (approx)
                let net = gross * fee_factor;
                let profit_after = (net - 1.0) * 100.0;

                if profit_after < min_profit_after {
                    continue;
                }

                // compute liquidity score: min of the three volumes (conservative)
                let v_ab = vol_map.get(a).and_then(|m| m.get(b)).copied().unwrap_or(0.0);
                let v_bc = vol_map.get(b).and_then(|m| m.get(c)).copied().unwrap_or(0.0);
                let v_ca = vol_map.get(c).and_then(|m| m.get(a)).copied().unwrap_or(0.0);
                let liquidity_score = v_ab.min(v_bc).min(v_ca);

                // dedupe: create canonical ordering (sorted triple) to avoid permutations
                let mut triple = vec![a.clone(), b.clone(), c.clone()];
                let key = {
                    // canonical unique orientation: choose lexicographically smallest rotation
                    let r1 = (triple[0].clone(), triple[1].clone(), triple[2].clone());
                    let r2 = (triple[1].clone(), triple[2].clone(), triple[0].clone());
                    let r3 = (triple[2].clone(), triple[0].clone(), triple[1].clone());
                    let mut rots = vec![r1, r2, r3];
                    rots.sort();
                    rots[0].clone()
                };

                if !seen.insert(key.clone()) {
                    continue;
                }

                let triangle_fmt = format!("{} → {} → {} → {}", a, b, c, a);
                let pairs_fmt = vec![
                    format!("{}/{}", a, b),
                    format!("{}/{}", b, c),
                    format!("{}/{}", c, a),
                ];

                out.push(TriangularResult{
                    triangle: triangle_fmt,
                    pairs: pairs_fmt,
                    profit_before: (profit_before as f64),
                    fees: total_fee_pct,
                    profit_after: (profit_after as f64),
                    score_liquidity: liquidity_score,
                });
            }
        }
    }

    // sort by profit after descending, then by liquidity descending
    out.sort_by(|x,y| {
        match y.profit_after.partial_cmp(&x.profit_after).unwrap_or(std::cmp::Ordering::Equal) {
            std::cmp::Ordering::Equal => y.score_liquidity.partial_cmp(&x.score_liquidity).unwrap_or(std::cmp::Ordering::Equal),
            ord => ord,
        }
    });

    out
        }
