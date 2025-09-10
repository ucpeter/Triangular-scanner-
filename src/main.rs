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
    // initialize tracing/logging
    utils::init_logging();

    // application state
    let app_state = AppState::default();
    let shared_state = Arc::new(TokioRwLock::new(app_state));

    // empty initial price cache
    let prices: SharedPrices = Arc::new(TokioRwLock::new(std::collections::HashMap::new()));

    // start WS workers (they write into ws_manager::GLOBAL_PRICES)
    ws_manager::start_all_workers(prices.clone()).await;

    // build router serving static UI and API endpoints
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let app = Router::new()
        .nest_service("/", ServeDir::new("static"))
        .route("/api/ui", get(routes::ui_handler))
        .route("/api/scan", post(routes::scan_handler))
        .route("/api/toggle", post(routes::toggle_handler))
        .with_state(shared_state.clone())
        .layer(cors);

    // bind & serve (use Render's PORT or 8080 locally)
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("invalid addr");

    info!("▶️  Starting server on http://0.0.0.0:{}", port);

    let listener = TcpListener::bind(addr).await.expect("failed to bind port");
    // axum::serve is available at top-level in axum 0.6/0.7; call it directly
    axum::serve(listener, app).await.expect("server error");
                                            }
