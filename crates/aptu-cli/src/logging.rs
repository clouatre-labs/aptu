// SPDX-License-Identifier: Apache-2.0

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
use tracing_subscriber::{EnvFilter, fmt};

/// Initialize the logging subsystem.
///
/// Verbosity levels:
/// - 0 (default): No tracing output (clean user-facing output)
/// - 1 (-v): No tracing output (verbose user info handled separately)
/// - 2+ (-vv): Full debug tracing with timestamps
///
/// The `RUST_LOG` environment variable can override these defaults.
pub fn init_logging(quiet: bool, verbosity: u8) {
    let fmt_layer = fmt::layer().with_target(false).with_writer(std::io::stderr);

    // Only enable tracing output for debug mode (verbosity >= 2)
    // Default and verbose modes use direct user output instead
    let default_filter = if verbosity >= 2 {
        "aptu=debug,octocrab=warn,reqwest=warn"
    } else if quiet {
        "aptu=error,octocrab=error,reqwest=error"
    } else {
        // Suppress tracing for default/verbose - user output is handled separately
        "aptu=error,octocrab=error,reqwest=error"
    };
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_filter))
        .expect("valid default filter directives");

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
