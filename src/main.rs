use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::net::TcpListener;

mod models;
mod exchanges;
mod logic;
mod utils;
mod routes;

#[tokio::main]
async fn main() {
    // init tracing/logger
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build app
    let app = Router::new()
        .merge(routes::routes()) // <-- routes.rs must provide pub fn routes() -> Router
        .nest_service("/", ServeDir::new("static"))
        .route("/health", get(|| async { "ok" }))
        .layer(CorsLayer::new().allow_origin(Any));

    // Port from env or default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8080);

    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("invalid addr");
    tracing::info!("Server listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await.expect("Failed to bind address");
    axum::serve(listener, app).await.expect("server error");
}
