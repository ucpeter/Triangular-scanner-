use crate::models::PairPrice;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularResult {
    pub triangle: (String, String, String),
    pub pairs: [PairPrice; 3],   // fixed: array instead of tuple
    pub profit_before: f64,
    pub fees: f64,
    pub profit_after: f64,
}

// ... keep your existing logic, just update push:
out.push(TriangularResult {
    triangle: (a.clone(), b.clone(), c.clone()),
    pairs: [pp1.clone(), pp2.clone(), pp3.clone()],
    profit_before,
    fees,
    profit_after,
});
