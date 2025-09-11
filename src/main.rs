use axum::{
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing_subscriber;

mod models;
mod routes;
mod exchanges;
mod logic;
mod utils;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Router: static frontend + API routes
    let app = Router::new()
        .nest_service("/", ServeDir::new("static"))
        .route("/scan", get(routes::scan));

    // Render sets PORT env var
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "10000".to_string())
        .parse()
        .expect("PORT must be a number");

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
