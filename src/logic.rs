// src/logic.rs
use crate::models::{PairPrice, ArbResult, PriceMap};
use std::collections::{HashMap, HashSet};
use crate::utils::round2;

/// Build price adjacency map from pair list
pub fn build_price_map(pairs: &[PairPrice]) -> PriceMap {
    let mut m: PriceMap = HashMap::new();
    for p in pairs {
        if !p.is_spot { continue; }
        if !(p.price.is_finite()) { continue; }
        if p.price <= 0.0 { continue; }
        let base = p.base.to_uppercase();
        let quote = p.quote.to_uppercase();
        m.entry(base.clone()).or_default().insert(quote.clone(), p.price);
        if p.price > 0.0 {
            m.entry(quote.clone()).or_default().insert(base.clone(), 1.0 / p.price);
        }
    }
    m
}

/// fee_per_leg percent (e.g., 0.1)
pub fn scan_triangles(price_map: &PriceMap, min_profit_before: f64, fee_per_leg: f64) -> Vec<ArbResult> {
    let mut out = Vec::new();
    let mut seen: HashSet<(String,String,String)> = HashSet::new();
    let fee_mult = 1.0 - fee_per_leg / 100.0;
    let total_fee_percent = 3.0 * fee_per_leg;

    for (a, bs) in price_map {
        for b in bs.keys() {
            if a == b { continue; }
            if let Some(cs) = price_map.get(b) {
                for c in cs.keys() {
                    if c == a || c == b { continue; }
                    if !price_map.get(c).map_or(false, |m| m.contains_key(a)) { continue; }

                    let r1 = *price_map.get(a).and_then(|m| m.get(b)).unwrap_or(&0.0);
                    let r2 = *price_map.get(b).and_then(|m| m.get(c)).unwrap_or(&0.0);
                    let r3 = *price_map.get(c).and_then(|m| m.get(a)).unwrap_or(&0.0);
                    if r1 <= 0.0 || r2 <= 0.0 || r3 <= 0.0 { continue; }

                    let gross = r1 * r2 * r3;
                    let profit_before = (gross - 1.0) * 100.0;
                    if !profit_before.is_finite() { continue; }
                    if profit_before < min_profit_before { continue; }

                    let net = (r1 * fee_mult) * (r2 * fee_mult) * (r3 * fee_mult);
                    let profit_after = (net - 1.0) * 100.0;

                    // canonical dedupe
                    let reps = vec![
                        (a.clone(), b.clone(), c.clone()),
                        (b.clone(), c.clone(), a.clone()),
                        (c.clone(), a.clone(), b.clone()),
                    ];
                    let key = reps.iter().min().unwrap().clone();
                    if !seen.insert(key) { continue; }

                    out.push(ArbResult {
                        route: format!("{} → {} → {} → {}", a, b, c, a),
                        pairs: format!("{}/{} | {}/{} | {}/{}", a, b, b, c, c, a),
                        profit_before: round2(profit_before),
                        fee_percent: round2(total_fee_percent),
                        profit_after: round2(profit_after),
                    });
                }
            }
        }
    }

    out.sort_by(|x,y| y.profit_after.partial_cmp(&x.profit_after).unwrap_or(std::cmp::Ordering::Equal));
    out
                        }
