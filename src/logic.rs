use crate::models::{PairPrice, TriangularResult};

/// Finds triangular arbitrage opportunities from a list of pairs.
/// Returns only those above `min_profit` percentage.
pub fn find_triangular_opportunities(
    pairs: &Vec<PairPrice>,
    min_profit: f64,
) -> Vec<TriangularResult> {
    let mut results = Vec::new();

    // Build lookup map for quick price access
    let mut map = std::collections::HashMap::new();
    for p in pairs {
        map.insert((p.base.clone(), p.quote.clone()), p.price);
    }

    // Brute-force triangular search: A/B, B/C, C/A
    for a in &map {
        let (base_a, quote_a) = (&(a.0).0, &(a.0).1);
        let price_a = a.1;

        for b in &map {
            let (base_b, quote_b) = (&(b.0).0, &(b.0).1);
            let price_b = b.1;

            if quote_a != base_b {
                continue;
            }

            for c in &map {
                let (base_c, quote_c) = (&(c.0).0, &(c.0).1);
                let price_c = c.1;

                if quote_b != base_c {
                    continue;
                }
                if quote_c != base_a {
                    continue;
                }

                // Simulate starting with 1 unit of base_a
                let amount = 1.0;
                let forward = amount * price_a * price_b * price_c;

                // Profit percentage
                let profit_pct = (forward - amount) / amount * 100.0;

                if profit_pct >= min_profit {
                    results.push(TriangularResult {
                        route: format!(
                            "{} -> {} -> {} -> {}",
                            base_a, quote_a, quote_b, quote_c
                        ),
                        profit_pct,
                    });
                }
            }
        }
    }

    results
            }
