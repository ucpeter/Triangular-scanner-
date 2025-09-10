mod models;
mod exchanges;
mod logic;
mod routes;
mod ws_manager;
mod utils;

use axum::{Router, routing::{get, post}};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::models::AppState;
use crate::ws_manager::SharedPrices;

#[tokio::main]
async fn main() {
    // initialize tracing logging (from utils)
    utils::init_logging();

    // application state (keeps last_results etc.)
    let app_state = AppState::default();
    let shared_state = Arc::new(TokioRwLock::new(app_state));

    // shared price cache (exchange -> Vec<PairPrice>) used by ws_manager
    let prices: SharedPrices = Arc::new(TokioRwLock::new(std::collections::HashMap::new()));

    // Start WebSocket workers in background (Binance, Bybit, KuCoin, Gate.io)
    // ws_manager::start_all_workers will spawn tasks and return immediately
    ws_manager::start_all_workers(prices.clone()).await;

    // serve static UI + API routes
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let app = Router::new()
        .nest_service("/", ServeDir::new("static"))
        .route("/api/ui", get(routes::ui_handler))
        .route("/api/scan", post(routes::scan_handler))
        .route("/api/toggle", post(routes::toggle_handler))
        .with_state(shared_state.clone())
        .layer(cors);

    // Listen on Render-provided PORT or 8080 locally
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("invalid addr");

    info!("▶️  Starting server on http://0.0.0.0:{}", port);

    // Bind TcpListener and hand it to axum::serve (axum 0.6 style)
    let listener = TcpListener::bind(addr).await.expect("failed to bind port");
    axum::serve(listener, app).await.expect("server error");
}
