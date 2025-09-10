use axum::{routing::{get, post}, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;

mod models;
mod routes;
mod logic;
mod exchanges;
mod ws_manager;
mod utils;

#[tokio::main]
async fn main() {
    utils::init_tracing();

    let app = Router::new()
        .route("/", get(routes::ui_handler))
        .route("/scan", post(routes::scan_handler));

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();

    tracing::info!("🚀 Starting WS scanner at http://{}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
