use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

mod routes;
mod exchanges;
mod models;
mod logic;
mod utils;

#[tokio::main]
async fn main() {
    // Initialize tracing for logs
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build router
    let app = Router::new()
        .route("/scan", post(routes::scan_handler))
        .route("/health", get(|| async { "ok" }))
        .nest_service("/", ServeDir::new("static"))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Bind to address
    let addr = SocketAddr::from(([0, 0, 0, 0], 10000));
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app)
        .await
        .expect("server error");
}
