use axum::{extract::Query, response::Json};
use serde::Deserialize;
use crate::{logic::find_triangular_arbitrage, exchanges::collect_prices};
use crate::models::TriangularResult;

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub exchanges: Option<String>,
    pub min_profit: Option<f64>,
}

pub async fn index() -> &'static str {
    "WS Arbitrage Scanner running"
}

pub async fn scan(Query(_req): Query<ScanRequest>) -> Json<Vec<TriangularResult>> {
    let prices = collect_prices().await;
    let results = find_triangular_arbitrage(prices);
    Json(results)
}
