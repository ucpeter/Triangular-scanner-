use crate::models::{PairPrice, TriangularResult};
use std::collections::{HashMap, HashSet};

/// Find triangular arbitrage opportunities.
pub fn find_triangular_opportunities(
    _exchange: &str,
    pairs: Vec<PairPrice>,
    min_profit_after: f64,
    fee_per_leg_pct: f64,   // now configurable
    neighbor_limit: usize,  // now configurable
) -> Vec<TriangularResult> {
    let mut adj: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let mut vol_map: HashMap<String, HashMap<String, f64>> = HashMap::new();

    for p in pairs.iter() {
        if !p.is_spot || !p.price.is_finite() || p.price <= 0.0 {
            continue;
        }
        let a = p.base.to_uppercase();
        let b = p.quote.to_uppercase();

        adj.entry(a.clone()).or_default().insert(b.clone(), p.price);
        if p.price > 0.0 && p.price.is_finite() {
            adj.entry(b.clone()).or_default().insert(a.clone(), 1.0 / p.price);
        }

        vol_map.entry(a.clone()).or_default().insert(b.clone(), p.volume);
        vol_map.entry(b.clone()).or_default().insert(a.clone(), p.volume);
    }

    let mut neighbors: HashMap<String, Vec<String>> = HashMap::new();
    for (base, targets) in adj.iter() {
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
        vv.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let list: Vec<String> = vv
            .into_iter()
            .take(neighbor_limit)
            .map(|(q, _)| q)
            .collect();
        neighbors.insert(base.clone(), list);
    }

    let mut preds: HashMap<String, HashSet<String>> = HashMap::new();
    for (u, m) in adj.iter() {
        for v in m.keys() {
            preds.entry(v.clone()).or_default().insert(u.clone());
        }
    }

    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    let mut out: Vec<TriangularResult> = Vec::new();

    let fee_factor = (1.0 - fee_per_leg_pct / 100.0).powi(3);
    let total_fee_pct = 3.0 * fee_per_leg_pct;

    for a in neighbors.keys() {
        let neigh_a = neighbors.get(a).cloned().unwrap_or_default();
        for b in neigh_a.iter() {
            if a == b {
                continue;
            }
            let nb = neighbors.get(b).cloned().unwrap_or_default();
            let pred_a = preds.get(a).cloned().unwrap_or_default();

            for c in nb.iter() {
                if c == a || c == b {
                    continue;
                }
                if !pred_a.contains(c) {
                    continue;
                }

                let r_ab = match adj.get(a).and_then(|m| m.get(b)) {
                    Some(&v) if v.is_finite() && v > 0.0 => v,
                    _ => continue,
                };
                let r_bc = match adj.get(b).and_then(|m| m.get(c)) {
                    Some(&v) if v.is_finite() && v > 0.0 => v,
                    _ => continue,
                };
                let r_ca = match adj.get(c).and_then(|m| m.get(a)) {
                    Some(&v) if v.is_finite() && v > 0.0 => v,
                    _ => continue,
                };

                let gross = r_ab * r_bc * r_ca;
                if !gross.is_finite() {
                    continue;
                }
                let profit_before = (gross - 1.0) * 100.0;
                if profit_before <= 0.0 {
                    continue;
                }

                let net = gross * fee_factor;
                let profit_after = (net - 1.0) * 100.0;
                if profit_after < min_profit_after {
                    continue;
                }

                let v_ab = vol_map.get(a).and_then(|m| m.get(b)).copied().unwrap_or(0.0);
                let v_bc = vol_map.get(b).and_then(|m| m.get(c)).copied().unwrap_or(0.0);
                let v_ca = vol_map.get(c).and_then(|m| m.get(a)).copied().unwrap_or(0.0);
                let liquidity_score = v_ab.min(v_bc).min(v_ca);

                let r1 = (a.clone(), b.clone(), c.clone());
                let r2 = (b.clone(), c.clone(), a.clone());
                let r3 = (c.clone(), a.clone(), b.clone());
                let mut rots = vec![r1, r2, r3];
                rots.sort();
                let key = rots[0].clone();

                if !seen.insert(key.clone()) {
                    continue;
                }

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
