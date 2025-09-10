use tracing_subscriber::EnvFilter;

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
