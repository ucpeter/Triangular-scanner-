mod models;
mod logic;
mod routes;
mod ws_manager;
mod utils;
mod exchanges;

use axum::{Router, routing::{get, post}, extract::State};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock as TokioRwLock;
use tower_http::services::ServeDir;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::models::AppState;
use crate::ws_manager::SharedPrices;

#[tokio::main]
async fn main() {
    // init logging
    utils::init_logging();

    // shared app state
    let app_state = AppState::default();
    let shared_state = Arc::new(TokioRwLock::new(app_state));

    // shared price cache
    let prices: SharedPrices = Arc::new(TokioRwLock::new(std::collections::HashMap::new()));

    // start ws workers
    ws_manager::start_all_workers(prices.clone()).await;

    // routes + static files
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let app = Router::new()
        .nest_service("/", ServeDir::new("static"))
        .route("/api/ui", get(routes::ui_handler))
        .route("/api/scan", post(routes::scan_handler))
        .route("/api/toggle", post(routes::toggle_handler))
        .with_state(shared_state.clone())
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();

    info!("Starting server at http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
