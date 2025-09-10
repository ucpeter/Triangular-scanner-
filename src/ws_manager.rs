use crate::models::PairPrice;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::RwLock;

pub static PRICE_CACHE: Lazy<Arc<RwLock<Vec<PairPrice>>>> =
    Lazy::new(|| Arc::new(RwLock::new(Vec::new())));

pub async fn gather_prices_for_exchanges(exchanges: &[String]) -> Result<Vec<PairPrice>, String> {
    let cache = PRICE_CACHE.read().await;
    if cache.is_empty() {
        return Err("Price cache empty".to_string());
    }
    Ok(cache.clone())
}
