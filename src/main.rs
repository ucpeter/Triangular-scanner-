mod models;
mod routes;
mod logic;
mod ws_manager;
mod utils;
mod exchanges;

use axum::{routing::{get, post}, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::cors::{Any, CorsLayer};
use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;
use crate::models::AppState;

#[tokio::main]
async fn main() {
    // init logging
    utils::init_tracing();

    // create shared app state (stores last scan results)
    let app_state = Arc::new(AppState::default());

    // start WS manager (it uses a global price cache inside ws_manager)
    // we pass an empty initial map (not required to hold anything)
    ws_manager::start_all_workers().await;

    // serve static files from ./static and API
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    let app = Router::new()
        .nest_service("/", ServeDir::new("static"))
        .route("/api/scan", post(routes::scan_handler))
        .route("/api/ui", get(routes::ui_handler))
        .with_state(app_state.clone())
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("invalid addr");

    tracing::info!("▶️  Starting server on http://0.0.0.0:{}", port);

    let listener = TcpListener::bind(addr).await.expect("failed to bind port");
    // axum::serve is present in axum 0.7
    axum::serve(listener, app).await.expect("server error");
}
