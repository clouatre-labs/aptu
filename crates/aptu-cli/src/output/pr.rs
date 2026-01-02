// SPDX-License-Identifier: Apache-2.0

//! PR review output rendering.

use std::io::{self, Write};

use console::style;

use crate::cli::OutputContext;
use crate::commands::types::{
    BulkPrReviewResult, PrLabelResult, PrReviewResult, SinglePrReviewOutcome,
};
use crate::output::Renderable;

impl Renderable for PrReviewResult {
    fn render_text(&self, w: &mut dyn Write, ctx: &OutputContext) -> io::Result<()> {
        // Verdict
        let verdict_style = match self.review.verdict.as_str() {
            "approve" => style(&self.review.verdict).green().bold(),
            "request_changes" => style(&self.review.verdict).red().bold(),
            _ => style(&self.review.verdict).yellow().bold(),
        };
        writeln!(w, "{}: {}", style("Verdict").bold(), verdict_style)?;
        writeln!(w)?;

        // Summary
        writeln!(w, "{}", style("Summary").cyan().bold())?;
        writeln!(w, "{}", self.review.summary)?;
        writeln!(w)?;

        // Strengths
        if !self.review.strengths.is_empty() {
            writeln!(w, "{}", style("Strengths").green().bold())?;
            for strength in &self.review.strengths {
                writeln!(w, "  + {strength}")?;
            }
            writeln!(w)?;
        }

        // Concerns
        if !self.review.concerns.is_empty() {
            writeln!(w, "{}", style("Concerns").red().bold())?;
            for concern in &self.review.concerns {
                writeln!(w, "  - {concern}")?;
            }
            writeln!(w)?;
        }

        // Line-level comments
        if !self.review.comments.is_empty() {
            writeln!(w, "{}", style("Comments").yellow().bold())?;
            for comment in &self.review.comments {
                let severity_style = match comment.severity.as_str() {
                    "issue" => style(&comment.severity).red(),
                    "warning" => style(&comment.severity).yellow(),
                    "suggestion" => style(&comment.severity).blue(),
                    _ => style(&comment.severity).dim(),
                };
                let line_info = comment.line.map_or(String::new(), |l| format!(":{l}"));
                writeln!(
                    w,
                    "  [{}] {}{}",
                    severity_style,
                    style(&comment.file).cyan(),
                    line_info
                )?;
                writeln!(w, "    {}", comment.comment)?;
            }
            writeln!(w)?;
        }

        // Suggestions
        if !self.review.suggestions.is_empty() {
            writeln!(w, "{}", style("Suggestions").blue().bold())?;
            for suggestion in &self.review.suggestions {
                writeln!(w, "  * {suggestion}")?;
            }
            writeln!(w)?;
        }

        // AI Stats (verbose only)
        if ctx.is_verbose() {
            writeln!(w, "{}", style("AI Stats").dim().bold())?;
            writeln!(
                w,
                "  Model: {} | Tokens: {} in, {} out | Duration: {}ms",
                self.ai_stats.model,
                self.ai_stats.input_tokens,
                self.ai_stats.output_tokens,
                self.ai_stats.duration_ms
            )?;
            if let Some(cost) = self.ai_stats.cost_usd {
                writeln!(w, "  Cost: ${cost:.6}")?;
            }
            writeln!(w)?;
        }

        // Dry-run message
        if self.dry_run {
            crate::output::common::show_dry_run_message(w, "DRY RUN MODE")?;
        }

        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## PR Review: #{} - {}", self.pr_number, self.pr_title)?;
        writeln!(w)?;
        writeln!(w, "**Verdict:** {}", self.review.verdict)?;
        writeln!(w)?;

        writeln!(w, "### Summary")?;
        writeln!(w, "{}", self.review.summary)?;
        writeln!(w)?;

        if !self.review.strengths.is_empty() {
            writeln!(w, "### Strengths")?;
            for strength in &self.review.strengths {
                writeln!(w, "- {strength}")?;
            }
            writeln!(w)?;
        }

        if !self.review.concerns.is_empty() {
            writeln!(w, "### Concerns")?;
            for concern in &self.review.concerns {
                writeln!(w, "- {concern}")?;
            }
            writeln!(w)?;
        }

        if !self.review.comments.is_empty() {
            writeln!(w, "### Comments")?;
            for comment in &self.review.comments {
                let line_info = comment.line.map_or(String::new(), |l| format!(":{l}"));
                writeln!(
                    w,
                    "- **[{}]** `{}{}`",
                    comment.severity, comment.file, line_info
                )?;
                writeln!(w, "  {}", comment.comment)?;
            }
            writeln!(w)?;
        }

        if !self.review.suggestions.is_empty() {
            writeln!(w, "### Suggestions")?;
            for suggestion in &self.review.suggestions {
                writeln!(w, "- {suggestion}")?;
            }
            writeln!(w)?;
        }

        Ok(())
    }
}

impl Renderable for BulkPrReviewResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", style("PR Review Summary").cyan().bold())?;
        writeln!(w)?;

        // Summary counts
        writeln!(
            w,
            "  {} succeeded | {} failed | {} skipped",
            style(self.succeeded).green().bold(),
            style(self.failed).red().bold(),
            style(self.skipped).yellow().bold(),
        )?;
        writeln!(w)?;

        // Per-PR outcomes
        if !self.outcomes.is_empty() {
            writeln!(w, "{}", style("Outcomes").dim().bold())?;
            for (pr_ref, outcome) in &self.outcomes {
                match outcome {
                    SinglePrReviewOutcome::Success(result) => {
                        writeln!(
                            w,
                            "  {} {} ({})",
                            style("✓").green(),
                            pr_ref,
                            style(&result.review.verdict).green()
                        )?;
                    }
                    SinglePrReviewOutcome::Skipped(reason) => {
                        writeln!(
                            w,
                            "  {} {} ({})",
                            style("⊘").yellow(),
                            pr_ref,
                            style(reason).yellow()
                        )?;
                    }
                    SinglePrReviewOutcome::Failed(error) => {
                        writeln!(
                            w,
                            "  {} {} ({})",
                            style("✗").red(),
                            pr_ref,
                            style(error).red()
                        )?;
                    }
                }
            }
            writeln!(w)?;
        }

        // Dry-run message
        if self.has_dry_run() {
            crate::output::common::show_dry_run_message(w, "DRY RUN MODE")?;
        }

        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## PR Review Summary")?;
        writeln!(w)?;

        writeln!(
            w,
            "- **Succeeded:** {}\n- **Failed:** {}\n- **Skipped:** {}",
            self.succeeded, self.failed, self.skipped
        )?;
        writeln!(w)?;

        if !self.outcomes.is_empty() {
            writeln!(w, "### Outcomes")?;
            for (pr_ref, outcome) in &self.outcomes {
                match outcome {
                    SinglePrReviewOutcome::Success(result) => {
                        writeln!(w, "- ✓ `{pr_ref}` ({})", result.review.verdict)?;
                    }
                    SinglePrReviewOutcome::Skipped(reason) => {
                        writeln!(w, "- ⊘ `{pr_ref}` ({reason})")?;
                    }
                    SinglePrReviewOutcome::Failed(error) => {
                        writeln!(w, "- ✗ `{pr_ref}` ({error})")?;
                    }
                }
            }
            writeln!(w)?;
        }

        Ok(())
    }
}

impl Renderable for PrLabelResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(
            w,
            "{} #{}: {}",
            style("PR").cyan().bold(),
            self.pr_number,
            style(&self.pr_title).bold()
        )?;
        writeln!(w, "{}", style(&self.pr_url).dim())?;
        writeln!(w)?;

        if self.dry_run {
            writeln!(w, "{}", style("DRY RUN MODE").yellow().bold())?;
            writeln!(w)?;
        }

        if self.labels.is_empty() {
            writeln!(w, "{}", style("No labels extracted").dim())?;
        } else {
            writeln!(w, "{}", style("Labels").cyan().bold())?;
            for label in &self.labels {
                writeln!(w, "  - {}", style(label).green())?;
            }
        }
        writeln!(w)?;

        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## PR Labels: #{} - {}", self.pr_number, self.pr_title)?;
        writeln!(w)?;

        if self.dry_run {
            writeln!(w, "**DRY RUN MODE**")?;
            writeln!(w)?;
        }

        if self.labels.is_empty() {
            writeln!(w, "No labels extracted")?;
        } else {
            writeln!(w, "### Labels")?;
            for label in &self.labels {
                writeln!(w, "- `{label}`")?;
            }
        }
        writeln!(w)?;

        Ok(())
    }
}
