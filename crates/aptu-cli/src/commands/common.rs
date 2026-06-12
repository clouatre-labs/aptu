// SPDX-License-Identifier: Apache-2.0

//! Shared helpers for CLI command handlers.

use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::OutputContext;

/// Creates a styled spinner (only if interactive).
pub fn maybe_spinner(ctx: &OutputContext, message: &str) -> Option<ProgressBar> {
    if ctx.is_interactive() {
        let s = ProgressBar::new_spinner();
        s.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg} ({elapsed:.cyan})")
                .expect("Invalid spinner template"),
        );
        s.set_message(message.to_string());
        s.enable_steady_tick(Duration::from_millis(100));
        Some(s)
    } else {
        None
    }
}
