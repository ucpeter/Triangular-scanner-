// src/logic.rs
use crate::models::{PairPrice, TriangularResult};
use std::collections::{HashMap, HashSet};


pub use crate::models::TriangularResult;
pub fn find_triangular_opportunities(
    _exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit_after: f64,
) -> Vec<TriangularResult> {
    // Default per-leg taker fee in percent (e.g. 0.10 means 0.10%)
    let fee_per_leg_pct: f64 = 0.10;
    // limit neighbors per node for performance
    let neighbor_limit: usize = 100;

    // adjacency: base -> map(quote -> price)
    let mut adj: HashMap<String, HashMap<String, f64>> = HashMap::new();
    // liquidity: base -> map(quote -> volume)
    let mut vol_map: HashMap<String, HashMap<String, f64>> = HashMap::new();

    // Build adjacency and volume maps
    for p in pairs.iter() {
        if !p.is_spot || !p.price.is_finite() || p.price <= 0.0 {
            continue;
        }
        let a = p.base.to_uppercase();
        let b = p.quote.to_uppercase();

        adj.entry(a.clone()).or_default().insert(b.clone(), p.price);
        // store inverse price (1 / price) for quick lookups
        if p.price > 0.0 && p.price.is_finite() {
            adj.entry(b.clone()).or_default().insert(a.clone(), 1.0 / p.price);
        }

        // store reported volume under both directions as approximation
        vol_map.entry(a.clone()).or_default().insert(b.clone(), p.volume);
        vol_map.entry(b.clone()).or_default().insert(a.clone(), p.volume);
    }

    // Build limited neighbor lists for each base (sorted by volume desc)
    let mut neighbors: HashMap<String, Vec<String>> = HashMap::new();
    for (base, targets) in adj.iter() {
        // collect (quote, volume)
        let mut vv: Vec<(String, f64)> = targets
            .keys()
            .map(|q| {
                let vol = vol_map
                    .get(base)
                    .and_then(|m| m.get(q))
                    .copied()
                    .unwrap_or(0.0);
                (q.clone(), vol)
            })
            .collect();

        // sort by volume desc
        vv.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let list: Vec<String> = vv.into_iter().take(neighbor_limit).map(|(q, _)| q).collect();
        neighbors.insert(base.clone(), list);
    }

    // Build predecessor sets: for a node X, preds[X] are nodes that have outgoing edge to X
    let mut preds: HashMap<String, HashSet<String>> = HashMap::new();
    for (u, m) in adj.iter() {
        for v in m.keys() {
            preds.entry(v.clone()).or_default().insert(u.clone());
        }
    }

    // dedupe set for canonical triangles
    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    let mut out: Vec<TriangularResult> = Vec::new();

    // multiplicative fee factor across 3 legs
    let fee_factor = (1.0 - fee_per_leg_pct / 100.0).powi(3);
    let total_fee_pct = 3.0 * fee_per_leg_pct;

    // Iterate A -> B -> C where C is neighbor of B and also predecessor of A
    for a in neighbors.keys() {
        // clone neighbor list to own the Strings
        let neigh_a = neighbors.get(a).cloned().unwrap_or_default();
        for b in neigh_a.iter() {
            if a == b {
                continue;
            }
            let nb = neighbors.get(b).cloned().unwrap_or_default();
            let pred_a = preds.get(a).cloned().unwrap_or_default();

            // build hashset for faster membership checks (owned Strings)
            let nb_set: HashSet<String> = nb.iter().cloned().collect();

            for c in nb.iter() {
                if c == a || c == b {
                    continue;
                }
                if !pred_a.contains(c) {
                    continue;
                }

                // lookup rates
                let r_ab = match adj.get(a).and_then(|m| m.get(b)) {
                    Some(&v) => v,
                    None => continue,
                };
                let r_bc = match adj.get(b).and_then(|m| m.get(c)) {
                    Some(&v) => v,
                    None => continue,
                };
                let r_ca = match adj.get(c).and_then(|m| m.get(a)) {
                    Some(&v) => v,
                    None => continue,
                };

                // gross multiplier
                let gross = r_ab * r_bc * r_ca;
                if !gross.is_finite() {
                    continue;
                }
                let profit_before = (gross - 1.0) * 100.0;
                if !profit_before.is_finite() || profit_before <= 0.0 {
                    continue;
                }

                // apply fees multiplicatively
                let net = gross * fee_factor;
                let profit_after = (net - 1.0) * 100.0;
                if !profit_after.is_finite() || profit_after < min_profit_after {
                    continue;
                }

                // liquidity score: conservative min of three volumes
                let v_ab = vol_map.get(a).and_then(|m| m.get(b)).copied().unwrap_or(0.0);
                let v_bc = vol_map.get(b).and_then(|m| m.get(c)).copied().unwrap_or(0.0);
                let v_ca = vol_map.get(c).and_then(|m| m.get(a)).copied().unwrap_or(0.0);
                let liquidity_score = v_ab.min(v_bc).min(v_ca);

                // canonical key (lexicographically smallest rotation) for dedupe
                let r1 = (a.clone(), b.clone(), c.clone());
                let r2 = (b.clone(), c.clone(), a.clone());
                let r3 = (c.clone(), a.clone(), b.clone());
                let mut rots = vec![r1, r2, r3];
                rots.sort();
                let key = rots[0].clone();

                if !seen.insert(key.clone()) {
                    continue;
                }

                // Prepare output fields
                let triangle_fmt = format!("{} → {} → {} → {}", a, b, c, a);
                let pairs_fmt = vec![
                    format!("{}/{}", a, b),
                    format!("{}/{}", b, c),
                    format!("{}/{}", c, a),
                ];

                out.push(TriangularResult {
                    triangle: triangle_fmt,
                    pairs: pairs_fmt,
                    profit_before,
                    fees: total_fee_pct,
                    profit_after,
                    score_liquidity: liquidity_score,
                });
            }
        }
    }

    // sort by profit_after desc, then liquidity desc
    out.sort_by(|x, y| {
        match y
            .profit_after
            .partial_cmp(&x.profit_after)
            .unwrap_or(std::cmp::Ordering::Equal)
        {
            std::cmp::Ordering::Equal => {
                y.score_liquidity
                    .partial_cmp(&x.score_liquidity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            ord => ord,
        }
    });

    out
        }
