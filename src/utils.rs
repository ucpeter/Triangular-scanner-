use tracing_subscriber::{EnvFilter, fmt};

pub fn init_tracing() {
    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter_layer)
        .init();
}
