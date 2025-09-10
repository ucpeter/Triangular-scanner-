use std::collections::HashMap;
use std::sync::Arc;
use once_cell::sync::Lazy;
use tokio::sync::RwLock as TokioRwLock;
use tracing::info;

use crate::models::PairPrice;

/// Shared prices type: exchange -> Vec<PairPrice>
pub type SharedPrices = Arc<TokioRwLock<HashMap<String, Vec<PairPrice>>>>;

/// Global shared prices (routes will read from this).
/// We keep a global here so routes can call `gather_prices_for_exchanges` without extra State typing.
pub static GLOBAL_PRICES: Lazy<SharedPrices> = Lazy::new(|| {
    Arc::new(TokioRwLock::new(HashMap::new()))
});

/// Spawn all WS workers (non-blocking). Each worker writes to `prices` keyed by exchange.
/// `initial` is optional initial price map (usually empty). start_all_workers will copy its content
/// into GLOBAL_PRICES and then spawn workers that write to GLOBAL_PRICES.
pub async fn start_all_workers(initial: SharedPrices) {
    // copy initial content (if any) to the global store
    {
        let mut g = GLOBAL_PRICES.write().await;
        let src = initial.read().await;
        *g = src.clone();
    }

    // spawn Binance
    {
        let prices = GLOBAL_PRICES.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::exchanges::binance::run_binance_ws(prices).await {
                tracing::error!("binance ws failed: {:?}", e);
            }
        });
    }

    // spawn Bybit
    {
        let prices = GLOBAL_PRICES.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::exchanges::bybit::run_bybit_ws(prices).await {
                tracing::error!("bybit ws failed: {:?}", e);
            }
        });
    }

    // spawn KuCoin
    {
        let prices = GLOBAL_PRICES.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::exchanges::kucoin::run_kucoin_ws(prices).await {
                tracing::error!("kucoin ws failed: {:?}", e);
            }
        });
    }

    // spawn Gate.io
    {
        let prices = GLOBAL_PRICES.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::exchanges::gateio::run_gateio_ws(prices).await {
                tracing::error!("gateio ws failed: {:?}", e);
            }
        });
    }

    info!("ws_manager: spawned all ws workers");
}

/// Gather live prices from the global SharedPrices map for a list of exchanges.
/// `exchanges` names are matched case-insensitively by lowercasing.
pub async fn gather_prices_for_exchanges(exchanges: &[String]) -> Result<Vec<PairPrice>, String> {
    let map = GLOBAL_PRICES.read().await;
    let mut merged: Vec<PairPrice> = Vec::new();

    for ex in exchanges {
        let key = ex.to_lowercase();
        if let Some(vec) = map.get(&key) {
            merged.extend_from_slice(vec);
        } else if let Some(vec) = map.get(ex) {
            // fallback: exact key
            merged.extend_from_slice(vec);
        }
    }

    Ok(merged)
    }
