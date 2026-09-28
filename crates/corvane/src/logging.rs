//! tracing setup: stderr in debug builds, daily-rolled file in ~/Library/Logs/Corvane.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_env("CORVANE_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,corvane=debug"));

    let logs_dir = corvane_platform::paths::logs_dir();
    let file_layer = std::fs::create_dir_all(&logs_dir).ok().map(|_| {
        let appender = tracing_appender::rolling::daily(&logs_dir, "corvane.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        (fmt::layer().with_ansi(false).with_writer(writer), guard)
    });

    let registry = tracing_subscriber::registry().with(filter);
    match file_layer {
        Some((layer, guard)) => {
            registry
                .with(layer)
                .with(cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr)))
                .init();
            Some(guard)
        }
        None => {
            registry
                .with(fmt::layer().with_writer(std::io::stderr))
                .init();
            None
        }
    }
}
