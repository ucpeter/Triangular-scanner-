mod models;
mod routes;
mod logic;
mod ws_manager;
mod utils;
mod exchanges;

use axum::{Router, routing::{get, post}, extract::State};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock as TokioRwLock;
use tower_http::cors::{CorsLayer, Any};
use tracing_subscriber;
use tracing::info;

use crate::models::AppState;
use crate::ws_manager::SharedPrices;

#[tokio::main]
async fn main() {
    // init logging
    tracing_subscriber::fmt::init();

    // shared app state
    let app_state = Arc::new(tokio::sync::Mutex::new(AppState::default()));

    // shared ws price map
    let shared_prices: SharedPrices = Arc::new(TokioRwLock::new(std::collections::HashMap::new()));

    // start WS workers (non-blocking)
    ws_manager::start_all_workers(shared_prices.clone()).await;

    // build routes
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let app = Router::new()
        .route("/", get(routes::ui_handler))
        .route("/scan", post(routes::scan_handler))
        .route("/toggle", post(routes::toggle_handler))
        .layer(cors)
        .with_state(app_state.clone());

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();

    info!("Starting WS arbitrage scanner at http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
