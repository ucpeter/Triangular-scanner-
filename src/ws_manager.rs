use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::models::PairPrice;
use crate::exchanges;

pub type SharedPrices = Arc<RwLock<HashMap<String, Vec<PairPrice>>>>;

/// start all exchange WS tasks (non-blocking)
pub async fn start_all_workers(prices: SharedPrices) {
    // spawn each exchange WS worker; each worker pushes into `prices` map keyed by exchange name
    // Binance implemented fully; other exchanges are implemented but may need slight parsing tweaks in production.
    let p1 = prices.clone();
    tokio::spawn(async move {
        if let Err(e) = exchanges::binance::run_binance_ws(p1.clone()).await {
            tracing::error!("binance ws failed: {:?}", e);
        }
    });

    let p2 = prices.clone();
    tokio::spawn(async move {
        if let Err(e) = exchanges::bybit::run_bybit_ws(p2.clone()).await {
            tracing::error!("bybit ws failed: {:?}", e);
        }
    });

    let p3 = prices.clone();
    tokio::spawn(async move {
        if let Err(e) = exchanges::kucoin::run_kucoin_ws(p3.clone()).await {
            tracing::error!("kucoin ws failed: {:?}", e);
        }
    });

    let p4 = prices.clone();
    tokio::spawn(async move {
        if let Err(e) = exchanges::gateio::run_gateio_ws(p4.clone()).await {
            tracing::error!("gateio ws failed: {:?}", e);
        }
    });

    info!("ws_manager started all workers");
  }
