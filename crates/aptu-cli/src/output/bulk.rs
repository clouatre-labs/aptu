// SPDX-License-Identifier: Apache-2.0

use comfy_table::{ContentArrangement, Table, presets::ASCII_MARKDOWN};
use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::types::{BulkTriageResult, SingleTriageOutcome};

use super::Renderable;

/// Format labels with + prefix for additions.
fn format_labels(labels: &[String]) -> String {
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels
            .iter()
            .map(|l| format!("+{l}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Format milestone with indicator if not set.
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

/// Collect dry-run outcomes from bulk result.
fn collect_dry_runs(result: &BulkTriageResult) -> Vec<&crate::commands::types::TriageResult> {
    result
        .outcomes
        .iter()
        .filter_map(|(_, outcome)| {
            if let SingleTriageOutcome::Success(triage_result) = outcome
                && triage_result.dry_run
            {
                return Some(triage_result.as_ref());
            }
            None
        })
        .collect()
}

/// Render dry-run summary table in text format using comfy-table.
fn render_dry_run_table(w: &mut dyn Write, result: &BulkTriageResult) -> io::Result<()> {
    let dry_runs = collect_dry_runs(result);
    if dry_runs.is_empty() {
        return Ok(());
    }

    writeln!(w, "{}", style("Proposed Changes (Dry Run)").cyan().bold())?;
    writeln!(w)?;

    let mut table = Table::new();
    table
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Issue", "Title", "Labels", "Milestone"]);

    for triage_result in dry_runs {
        table.add_row(vec![
            format!("#{}", triage_result.issue_number),
            triage_result.issue_title.clone(),
            format_labels(&triage_result.triage.suggested_labels),
            format_milestone(triage_result.triage.suggested_milestone.as_ref()),
        ]);
    }

    writeln!(w, "{table}")?;
    writeln!(w)?;
    Ok(())
}

/// Render dry-run summary table in markdown format.
fn render_dry_run_markdown_table(w: &mut dyn Write, result: &BulkTriageResult) -> io::Result<()> {
    let dry_runs = collect_dry_runs(result);
    if dry_runs.is_empty() {
        return Ok(());
    }

    writeln!(w, "### Proposed Changes (Dry Run)")?;
    writeln!(w)?;

    let mut table = Table::new();
    table
        .load_preset(ASCII_MARKDOWN)
        .set_header(vec!["Issue", "Title", "Labels", "Milestone"]);

    for triage_result in dry_runs {
        table.add_row(vec![
            format!("#{}", triage_result.issue_number),
            triage_result.issue_title.clone(),
            format_labels(&triage_result.triage.suggested_labels),
            format_milestone(triage_result.triage.suggested_milestone.as_ref()),
        ]);
    }

    writeln!(w, "{table}")?;
    writeln!(w)?;
    Ok(())
}
