// src/utils.rs
use tracing_subscriber::prelude::*; // brings SubscriberExt (the .with() method) into scope
use tracing_subscriber::{fmt, EnvFilter, Registry};

/// Initialize global tracing/logging for the app.
///
/// Usage: call `utils::init_tracing()` early in main().
pub fn init_tracing() {
    // formatting layer (no target to reduce verbosity)
    let fmt_layer = fmt::layer().with_target(false);

    // allow overriding via RUST_LOG / default to "info"
    let filter_layer = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Registry + layers; `.with` is available thanks to prelude::*
    Registry::default()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
