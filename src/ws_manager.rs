use crate::models::PairPrice;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Shared price cache type: exchange -> Vec<PairPrice>
pub type SharedPrices = Arc<RwLock<HashMap<String, Vec<PairPrice>>>>;

/// Global price cache used by workers and API
pub static GLOBAL_PRICES: Lazy<SharedPrices> = Lazy::new(|| {
    Arc::new(RwLock::new(HashMap::new()))
});

/// Start all exchange WS workers.
/// Each worker writes into GLOBAL_PRICES keyed by exchange name (lowercase).
pub async fn start_all_workers() {
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

/// Gather prices (merged) for a list of exchange names.
/// exchange names are lowercased when matched.
pub async fn gather_prices_for_exchanges(exchanges: &[String]) -> Result<Vec<PairPrice>, String> {
    let guard = GLOBAL_PRICES.read().await;
    let mut out: Vec<PairPrice> = Vec::new();
    for name in exchanges {
        let key = name.to_lowercase();
        if let Some(vec) = guard.get(&key) {
            out.extend_from_slice(vec);
        }
    }
    Ok(out)
                    }
