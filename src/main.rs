mod models;
mod exchanges;
mod logic;
mod routes;
mod ws_manager;
mod utils;

use axum::{routing::{get, post}, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

use crate::models::AppState;

#[tokio::main]
async fn main() {
    // ✅ Shared state with RwLock for thread-safe reads/writes
    let shared_state = Arc::new(AppState {
        last_results: Arc::new(RwLock::new(None)),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // ✅ Setup routes
    let app = Router::new()
        .route("/", get(routes::ui_handler))
        .route("/scan", post(routes::scan_handler))
        .with_state(shared_state.clone())
        .layer(cors);

    // ✅ Port binding (Render defaults to $PORT)
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");

    println!("🚀 WS Triangular Arbitrage Scanner running at http://{}", addr);

    // ✅ Launch Axum server
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
                    }
