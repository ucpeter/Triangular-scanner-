use std::{net::SocketAddr, sync::Arc, collections::HashMap};

use axum::{
    routing::{get, post},
    Router,
};
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

mod models;
mod routes;
mod logic;
mod exchanges;
mod utils;

use models::{AppState, SharedPrices};
use routes::scan_handler;

#[tokio::main]
async fn main() {
    // init logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Shared state
    let state = AppState {
        prices: Arc::new(RwLock::new(HashMap::new())),
    };

    // spawn exchange WS clients in background (Binance, Bybit, Kucoin, Gateio)
    let prices_clone = state.prices.clone();
    tokio::spawn(async move {
        exchanges::binance::run_binance_ws(prices_clone).await.ok();
    });

    let prices_clone = state.prices.clone();
    tokio::spawn(async move {
        exchanges::bybit::run_bybit_ws(prices_clone).await.ok();
    });

    let prices_clone = state.prices.clone();
    tokio::spawn(async move {
        exchanges::kucoin::run_kucoin_ws(prices_clone).await.ok();
    });

    let prices_clone = state.prices.clone();
    tokio::spawn(async move {
        exchanges::gateio::run_gateio_ws(prices_clone).await.ok();
    });

    // build routes
    let app = Router::new()
        .route("/scan", post(scan_handler))
        .route_service("/", ServeDir::new("static"))
        .with_state(state);

    // start server
    let addr: SocketAddr = "0.0.0.0:10000".parse().unwrap();
    tracing::info!("🚀 server running at http://{}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
        }
