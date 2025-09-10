use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::models::PairPrice;

/// shared prices type: exchange -> Vec<PairPrice>
pub type SharedPrices = Arc<RwLock<HashMap<String, Vec<PairPrice>>>>;

/// spawn all WS workers (non-blocking). Each worker writes to `prices` keyed by exchange.
pub async fn start_all_workers(prices: SharedPrices) {
    // spawn Binance
    {
        let p = prices.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::exchanges::binance::run_binance_ws(p).await {
                tracing::error!("binance ws failed: {:?}", e);
            }
        });
    }

    // spawn Bybit
    {
        let p = prices.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::exchanges::bybit::run_bybit_ws(p).await {
                tracing::error!("bybit ws failed: {:?}", e);
            }
        });
    }

    // spawn KuCoin
    {
        let p = prices.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::exchanges::kucoin::run_kucoin_ws(p).await {
                tracing::error!("kucoin ws failed: {:?}", e);
            }
        });
    }

    // spawn Gate.io
    {
        let p = prices.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::exchanges::gateio::run_gateio_ws(p).await {
                tracing::error!("gateio ws failed: {:?}", e);
            }
        });
    }

    info!("ws_manager: spawned all ws workers");
                }
