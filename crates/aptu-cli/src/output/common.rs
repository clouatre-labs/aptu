// SPDX-License-Identifier: Apache-2.0

//! Common UX helper functions for consistent display patterns across commands.

use console::style;
use std::io::Write;

use super::OutputContext;
use crate::cli::OutputFormat;

/// Display progress indicator for bulk operations.
///
/// Shows "[current/total] Action" in cyan bold when in text format.
///
/// # Arguments
/// * `ctx` - Output context to determine format
/// * `current` - Current item number (1-indexed)
/// * `total` - Total number of items
/// * `action` - Action description (e.g., "Triaging", "Reviewing")
pub fn show_progress(ctx: &OutputContext, current: usize, total: usize, action: &str) {
    if matches!(ctx.format, OutputFormat::Text) {
        println!("\n[{}/{}] {}", current, total, style(action).cyan().bold());
    }
}

/// Display preview of an issue or PR with title and labels.
///
/// Shows styled title and labels in text format. Labels are displayed as
/// comma-separated cyan text, or "none" if empty.
///
/// # Arguments
/// * `ctx` - Output context to determine format
/// * `title` - Issue or PR title
/// * `labels` - List of label names
pub fn show_preview(ctx: &OutputContext, title: &str, labels: &[String]) {
    if matches!(ctx.format, OutputFormat::Text) {
        println!("  {}  {}", style("title:").dim(), style(title).bold());
        let labels_display = if labels.is_empty() {
            style("none").dim().to_string()
        } else {
            labels
                .iter()
                .map(|l| style(l).cyan().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!("  {}  {}", style("labels:").dim(), labels_display);
        println!();
    }
}

/// Display dry-run message to a writer.
///
/// Writes a yellow styled message indicating the operation was not performed.
///
/// # Arguments
/// * `w` - Writer to output to
/// * `message` - Message to display (e.g., "Dry run - comment not posted.")
///
/// # Errors
/// Returns error if write operation fails.
pub fn show_dry_run_message<W: Write + ?Sized>(w: &mut W, message: &str) -> std::io::Result<()> {
    writeln!(w, "{}", style(message).yellow())
}

/// Display timing information for fetch and AI analysis.
///
/// Shows fetch time and AI analysis stats (model, duration, tokens) when
/// verbose mode is enabled and in text format.
///
/// # Arguments
/// * `ctx` - Output context to check verbose and format settings
/// * `fetch_ms` - Fetch duration in milliseconds
/// * `model` - AI model name
/// * `duration_ms` - AI analysis duration in milliseconds
/// * `input_tokens` - Number of input tokens
/// * `output_tokens` - Number of output tokens
pub fn show_timing(
    ctx: &OutputContext,
    fetch_ms: u128,
    model: &str,
    duration_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
) {
    if ctx.is_verbose() && matches!(ctx.format, OutputFormat::Text) {
        println!(
            "  {}",
            style(format!("Fetched issue in {fetch_ms}ms")).dim()
        );

        #[allow(clippy::cast_precision_loss)]
        let duration_secs = duration_ms as f64 / 1000.0;
        let total_tokens = input_tokens + output_tokens;
        println!(
            "  {} (model: {}) in {:.1}s ({} tokens)",
            style("AI analysis").dim(),
            style(model).cyan(),
            duration_secs,
            total_tokens
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_progress_text_format() {
        let ctx = OutputContext {
            format: OutputFormat::Text,
            verbosity: 0,
            is_tty: true,
            quiet: false,
        };
        // Visual test - should print "[2/5] Processing"
        show_progress(&ctx, 2, 5, "Processing");
    }

    #[test]
    fn test_show_progress_json_format() {
        let ctx = OutputContext {
            format: OutputFormat::Json,
            verbosity: 0,
            is_tty: false,
            quiet: false,
        };
        // Should not print anything
        show_progress(&ctx, 1, 1, "Test");
    }

    #[test]
    fn test_show_preview_with_labels() {
        let ctx = OutputContext {
            format: OutputFormat::Text,
            verbosity: 0,
            is_tty: true,
            quiet: false,
        };
        let labels = vec!["bug".to_string(), "help wanted".to_string()];
        show_preview(&ctx, "Test Issue", &labels);
    }

    #[test]
    fn test_show_preview_no_labels() {
        let ctx = OutputContext {
            format: OutputFormat::Text,
            verbosity: 0,
            is_tty: true,
            quiet: false,
        };
        show_preview(&ctx, "Test Issue", &[]);
    }

    #[test]
    fn test_show_preview_json_format() {
        let ctx = OutputContext {
            format: OutputFormat::Json,
            verbosity: 0,
            is_tty: false,
            quiet: false,
        };
        show_preview(&ctx, "Test", &["label".to_string()]);
    }

    #[test]
    fn test_show_dry_run_message() {
        let mut buf = Vec::new();
        show_dry_run_message(&mut buf, "Dry run - comment not posted.").unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Dry run - comment not posted."));
    }

    #[test]
    fn test_show_timing_verbose() {
        let ctx = OutputContext {
            format: OutputFormat::Text,
            verbosity: 1,
            is_tty: true,
            quiet: false,
        };
        show_timing(&ctx, 150, "gpt-4", 2500, 100, 50);
    }

    #[test]
    fn test_show_timing_quiet() {
        let ctx = OutputContext {
            format: OutputFormat::Text,
            verbosity: 0,
            is_tty: true,
            quiet: true,
        };
        // Should not print anything
        show_timing(&ctx, 150, "gpt-4", 2500, 100, 50);
    }

    #[test]
    fn test_show_timing_json_format() {
        let ctx = OutputContext {
            format: OutputFormat::Json,
            verbosity: 1,
            is_tty: false,
            quiet: false,
        };
        // Should not print anything
        show_timing(&ctx, 150, "gpt-4", 2500, 100, 50);
    }
}
