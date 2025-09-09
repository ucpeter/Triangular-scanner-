use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tracing::{info, error};
use tower_http::cors::{Any, CorsLayer};

mod models;
mod routes;
mod logic;
mod ws_manager;
mod utils; // ✅ keep utils.rs in scope
mod exchanges;

use models::{AppState, ExecMode};
use routes::{scan_handler, toggle_mode_handler};

#[tokio::main]
async fn main() {
    // ✅ initialize logging from utils
    utils::init_logging();

    // Broadcast channel for pushing WS updates
    let (tx, _rx) = broadcast::channel(100);

    let state = AppState {
        mode: ExecMode::RequestOnly, // default mode
        tx,
    };

    // Router
    let app = Router::new()
        .route("/scan", post(scan_handler))
        .route("/toggle", post(toggle_mode_handler))
        .route("/", get(|| async { "✅ WS Triangular Arbitrage Scanner running" }))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Bind address
    let addr = SocketAddr::from(([0, 0, 0, 0], 10000));
    info!("🚀 Server running on http://{}", addr);

    // Run server
    if let Err(e) = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
    {
        error!("❌ Server failed: {}", e);
    }
        }
