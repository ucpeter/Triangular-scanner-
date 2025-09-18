use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod exchanges;
mod logic;
mod models;
mod routes;
mod utils;

#[tokio::main]
async fn main() {
    // Setup tracing subscriber
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_filter(tracing_subscriber::filter::LevelFilter::from_level(Level::INFO)),
        )
        .init();

    // Build application with routes
    let app = Router::new()
        .merge(routes::routes())
        .route("/", get(root_handler))
        .layer(TraceLayer::new_for_http());

    // Bind server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("🚀 Triangular Scanner running at http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("Server crashed");
}

async fn root_handler() -> &'static str {
    "Triangular Arbitrage Scanner API is running.\nTry POST /scan"
}
