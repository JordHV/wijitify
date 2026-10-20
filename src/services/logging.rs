use std::panic;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(false).init();

    panic::set_hook(Box::new(|info| {
        tracing::error!("panic: {info}");
    }));
}
