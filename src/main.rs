// src/main.rs
mod models;
mod exchanges;
mod logic;
mod routes;
mod utils;

use axum::{routing::{get, post}, Router};
use hyper::Server;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt};
use std::env;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let state = Arc::new(());

    let app = Router::new()
        .route("/", get(routes::ui_handler))
        .route("/scan", post(routes::scan_handler))
        .with_state(state)
        .merge(Router::new().nest_service("/static", ServeDir::new("static")));

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("invalid addr");

    println!("Starting server on http://0.0.0.0:{}", port);
    Server::bind(&addr).serve(app.into_make_service()).await.expect("server error");
}
