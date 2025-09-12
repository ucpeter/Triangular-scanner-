use axum::{Router, routing::get};
use tower_http::services::ServeDir;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod models;
mod exchanges;
mod arbitrage;
mod utils;
mod routes;

#[tokio::main]
async fn main() {
    // Init tracing logs
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build app with /scan and static UI
    let app = Router::new()
        .merge(routes::routes())
        .nest_service("/", ServeDir::new("static"))
        .route("/health", get(|| async { "ok" }));

    // Bind to port
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("Server listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
