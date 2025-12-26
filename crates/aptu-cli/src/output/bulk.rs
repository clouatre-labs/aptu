// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::types::{BulkTriageResult, SingleTriageOutcome};

use super::Renderable;

/// Truncate a string to a maximum width, respecting UTF-8 boundaries.
fn truncate_string(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else {
        s.chars()
            .take(max_width.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

/// Format labels with + prefix for additions, truncating if needed.
fn format_labels(labels: &[String], max_width: Option<usize>) -> String {
    let formatted = if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels
            .iter()
            .map(|l| format!("+{l}"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    match max_width {
        Some(width) => truncate_string(&formatted, width),
        None => formatted,
    }
}

/// Format milestone with (no change) indicator if not set.
fn format_milestone(milestone: Option<&String>) -> String {
    match milestone {
        Some(m) => format!("+{m}"),
        None => "(no change)".to_string(),
    }
}

impl Renderable for BulkTriageResult {
    fn render_text(&self, w: &mut dyn Write, ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", style("Bulk Triage Summary").bold().green())?;
        writeln!(w, "{}", style("=".repeat(20)).dim())?;
        writeln!(w, "  Succeeded: {}", style(self.succeeded).green())?;
        writeln!(w, "  Failed:    {}", style(self.failed).red())?;
        writeln!(w, "  Skipped:   {}", style(self.skipped).yellow())?;
        writeln!(
            w,
            "  Total:     {}",
            self.succeeded + self.failed + self.skipped
        )?;
        writeln!(w)?;

        // Render summary table if dry-run and interactive
        if self.has_dry_run() && ctx.is_interactive() {
            render_dry_run_table(w, self)?;
        }

        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "## Bulk Triage Summary")?;
        writeln!(w)?;
        writeln!(w, "- Succeeded: {}", self.succeeded)?;
        writeln!(w, "- Failed: {}", self.failed)?;
        writeln!(w, "- Skipped: {}", self.skipped)?;
        writeln!(
            w,
            "- Total: {}",
            self.succeeded + self.failed + self.skipped
        )?;
        writeln!(w)?;

        // Render markdown table if dry-run
        if self.has_dry_run() {
            render_dry_run_markdown_table(w, self)?;
        }

        Ok(())
    }
}

/// Render dry-run summary table in text format.
fn render_dry_run_table(w: &mut dyn Write, result: &BulkTriageResult) -> io::Result<()> {
    writeln!(w, "{}", style("Proposed Changes (Dry Run)").cyan().bold())?;
    writeln!(w, "{}", style("-".repeat(80)).dim())?;

    // Collect dry-run outcomes
    let dry_runs: Vec<_> = result
        .outcomes
        .iter()
        .filter_map(|(_, outcome)| {
            if let SingleTriageOutcome::Success(triage_result) = outcome
                && triage_result.dry_run
            {
                return Some(triage_result);
            }
            None
        })
        .collect();

    if dry_runs.is_empty() {
        return Ok(());
    }

    // Header
    writeln!(
        w,
        "{:<10} {:<30} {:<20} {:<15}",
        "Issue", "Title", "Labels", "Milestone"
    )?;
    writeln!(w, "{}", style("-".repeat(80)).dim())?;

    // Rows
    for triage_result in dry_runs {
        let issue_num = triage_result.issue_number.to_string();
        let title = truncate_string(&triage_result.issue_title, 28);
        let labels = format_labels(&triage_result.triage.suggested_labels, Some(25));
        let milestone = format_milestone(triage_result.triage.suggested_milestone.as_ref());

        writeln!(w, "{issue_num:<8} {title:<30} {labels:<27} {milestone}")?;
    }

    writeln!(w)?;
    Ok(())
}

/// Render dry-run summary table in markdown format.
fn render_dry_run_markdown_table(w: &mut dyn Write, result: &BulkTriageResult) -> io::Result<()> {
    writeln!(w, "### Proposed Changes (Dry Run)")?;
    writeln!(w)?;

    // Collect dry-run outcomes
    let dry_runs: Vec<_> = result
        .outcomes
        .iter()
        .filter_map(|(_, outcome)| {
            if let SingleTriageOutcome::Success(triage_result) = outcome
                && triage_result.dry_run
            {
                return Some(triage_result);
            }
            None
        })
        .collect();

    if dry_runs.is_empty() {
        return Ok(());
    }

    // Markdown table header
    writeln!(w, "| Issue | Title | Labels | Milestone |")?;
    writeln!(w, "|-------|-------|--------|-----------|")?;

    // Rows
    for triage_result in dry_runs {
        let issue_num = triage_result.issue_number;
        let title = truncate_string(&triage_result.issue_title, 30);
        let labels = format_labels(&triage_result.triage.suggested_labels, None);
        let milestone = format_milestone(triage_result.triage.suggested_milestone.as_ref());

        writeln!(w, "| #{issue_num} | {title} | {labels} | {milestone} |")?;
    }

    writeln!(w)?;
    Ok(())
}
