//! Logging initialization for the Aptu CLI.
//!
//! Uses `tracing` with `tracing-subscriber` for structured logging.
//! Log level can be controlled via the `RUST_LOG` environment variable.
//!
//! # Examples
//!
//! ```bash
//! # Default: info level for aptu, warn for dependencies
//! cargo run
//!
//! # Debug output for troubleshooting
//! RUST_LOG=aptu=debug cargo run
//!
//! # Trace level for verbose debugging
//! RUST_LOG=aptu=trace cargo run
//! ```

use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// Initialize the logging subsystem.
///
/// Sets up `tracing` with the following defaults:
/// - `aptu=info` - Info level for Aptu code
/// - `octocrab=warn` - Warn level for GitHub API client
/// - `reqwest=warn` - Warn level for HTTP client
///
/// These defaults can be overridden via the `RUST_LOG` environment variable.
pub fn init_logging(quiet: bool) {
    let fmt_layer = fmt::layer().with_target(false).with_writer(std::io::stderr);

    let default_filter = if quiet {
        "aptu=warn,octocrab=warn,reqwest=warn"
    } else {
        "aptu=info,octocrab=warn,reqwest=warn"
    };
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_filter))
        .expect("valid default filter directives");

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
