// SPDX-License-Identifier: Apache-2.0

//! Logging initialization for the Aptu CLI.
//!
//! Uses `tracing` with `tracing-subscriber` for structured logging.
//! Log level can be controlled via the `RUST_LOG` environment variable.
//!
//! The `-v` flag controls user-facing verbose output (handled separately by `OutputContext`).
//! For debug-level tracing, use the `RUST_LOG` environment variable.
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

use crate::cli::OutputFormat;

/// Initialize the logging subsystem.
///
/// The `verbose` flag controls user-facing output verbosity (handled separately by `OutputContext`).
/// The `RUST_LOG` environment variable controls debug tracing output.
///
/// # Metrics Collection
///
/// To enable metrics collection, set `RUST_LOG=aptu=info`:
///
/// ```bash
/// RUST_LOG=aptu=info cargo run -- triage <owner>/<repo>/<issue_num>
/// ```
///
/// Metrics are emitted as structured logs containing:
/// - `duration_ms`: Total API request time in milliseconds
/// - `input_tokens`: Number of input tokens used
/// - `output_tokens`: Number of output tokens used
/// - `cost_usd`: Estimated cost in USD (if available)
/// - `model`: AI model name
///
/// Pipe to jq to filter metrics:
///
/// ```bash
/// RUST_LOG=aptu=info cargo run -- triage <owner>/<repo>/<issue_num> 2>&1 | jq 'select(.fields.duration_ms)'
/// ```
///
/// # Arguments
///
/// * `format` - Output format (determines if quiet mode is enabled)
/// * `verbose` - Whether verbose user output is enabled (-v flag)
pub fn init_logging(format: OutputFormat, _verbose: bool) {
    let fmt_layer = fmt::layer().with_target(false).with_writer(std::io::stderr);

    // Derive quiet mode from format (structured formats are quiet)
    let quiet = matches!(
        format,
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Markdown
    );

    // Default filter: suppress tracing unless RUST_LOG is set
    // Users can enable info-level metrics with RUST_LOG=aptu=info
    // Users can enable debug output with RUST_LOG=aptu=debug
    let default_filter = if quiet {
        "aptu=warn,octocrab=error,reqwest=error"
    } else {
        // Suppress tracing for default/verbose - user output is handled separately
        "aptu=warn,octocrab=error,reqwest=error"
    };
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_filter))
        .expect("valid default filter directives");

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
