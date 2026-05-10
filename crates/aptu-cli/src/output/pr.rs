// SPDX-License-Identifier: Apache-2.0

//! PR review output rendering.

use std::io::{self, Write};

use console::style;
use serde::Serialize;

use crate::cli::OutputContext;
use crate::commands::types::{
    BulkPrReviewResult, PrLabelResult, PrReviewResult, SinglePrReviewOutcome,
};
use crate::output::Renderable;
use aptu_core::PrCreateResult;

/// Maximum title length in characters for text table output.
const QUEUE_TITLE_MAX_CHARS: usize = 50;

/// A single PR in the queue result.
#[derive(Debug, Clone, Serialize)]
pub struct QueuedPr {
    /// PR number
    pub number: u64,
    /// PR title
    pub title: String,
    /// Author login
    pub author: String,
    /// Age in days
    pub age_days: f64,
    /// Additions
    pub additions: u64,
    /// Deletions
    pub deletions: u64,
    /// Reviewability score (0.0-1.0)
    pub score: f64,
    /// Is draft
    pub draft: bool,
}

/// Result from `pr queue` command.
#[derive(Debug, Serialize)]
pub struct PrQueueResult {
    /// Queued PRs sorted by score DESC
    pub prs: Vec<QueuedPr>,
    /// Total number of open PRs (including drafts)
    pub total_open: usize,
    /// Number of draft PRs excluded
    pub drafts_excluded: usize,
}

/// Format age in days as "Xd" or "Xmo".
fn format_age(age_days: f64) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    if age_days < 30.0 {
        format!("{}d", age_days.round() as u32)
    } else if age_days < 365.0 {
        format!("{}mo", (age_days / 30.0).round() as u32)
    } else {
        format!("{}y", (age_days / 365.0).round() as u32)
    }
}

impl Renderable for PrQueueResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        if self.prs.is_empty() {
            writeln!(w, "{}", style("No open PRs found.").yellow())?;
            return Ok(());
        }

        writeln!(w)?;
        writeln!(
            w,
            "{}",
            style("Open PRs ranked by reviewability (size 60%, age 40%)".to_string()).bold()
        )?;
        writeln!(w)?;

        // Header
        writeln!(
            w,
            "{}",
            style(format!(
                // Column widths: title=QUEUE_TITLE_MAX_CHARS, author=20, age=8, changes=12, score=3
                "{:>4}  {:<50}  {:<20}  {:<8}  {:<12}  {:>3}",
                "Rank", "Title", "Author", "Age", "Changes", "Score"
            ))
            .bold()
        )?;
        writeln!(w, "{}", style("-".repeat(120)).dim())?;

        for (idx, pr) in self.prs.iter().enumerate() {
            let rank = idx + 1;
            let title = aptu_core::utils::truncate(&pr.title, QUEUE_TITLE_MAX_CHARS);
            let age_str = format_age(pr.age_days);
            let changes = format!("+{}-{}", pr.additions, pr.deletions);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let score_int = (pr.score * 100.0).round() as u32;

            writeln!(
                w,
                "{:>4}  {:<50}  {:<20}  {:<8}  {:<12}  {:>3}",
                rank, title, pr.author, age_str, changes, score_int
            )?;
        }

        writeln!(w)?;
        let footer = format!(
            "Showing {} of {} open PR{} ({} draft{} excluded)",
            self.prs.len(),
            self.total_open,
            if self.total_open == 1 { "" } else { "s" },
            self.drafts_excluded,
            if self.drafts_excluded == 1 { "" } else { "s" }
        );
        writeln!(w, "{}", style(footer).dim())?;

        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        if self.prs.is_empty() {
            writeln!(w, "No open PRs found.")?;
            return Ok(());
        }

        writeln!(w, "## Pull Request Queue\n")?;
        writeln!(w, "Ranked by reviewability score (60% size, 40% age)\n")?;

        writeln!(w, "| Rank | Title | Author | Age | Changes | Score |")?;
        writeln!(w, "|------|-------|--------|-----|---------|-------|")?;

        for (idx, pr) in self.prs.iter().enumerate() {
            let rank = idx + 1;
            let title = aptu_core::utils::truncate(&pr.title, QUEUE_TITLE_MAX_CHARS);
            let age_str = format_age(pr.age_days);
            let changes = format!("+{}-{}", pr.additions, pr.deletions);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let score_int = (pr.score * 100.0).round() as u32;

            writeln!(
                w,
                "| {} | {} | {} | {} | {} | {} |",
                rank, title, pr.author, age_str, changes, score_int
            )?;
        }

        writeln!(w)?;
        writeln!(
            w,
            "Showing {} of {} open PR{} ({} draft{} excluded)",
            self.prs.len(),
            self.total_open,
            if self.total_open == 1 { "" } else { "s" },
            self.drafts_excluded,
            if self.drafts_excluded == 1 { "" } else { "s" }
        )?;

        Ok(())
    }
}

fn render_security_findings_text(
    w: &mut dyn Write,
    findings: &[aptu_core::Finding],
    ctx: &OutputContext,
) -> io::Result<()> {
    if findings.is_empty() {
        // No findings - show clean status
        writeln!(
            w,
            "{}: {}",
            style("Security Scan").green().bold(),
            style("No issues found").green()
        )?;
        writeln!(w)?;
    } else if ctx.verbose {
        // Verbose mode: show all findings with details
        writeln!(w, "{}", style("Security Findings").red().bold())?;
        for finding in findings {
            let severity_style = match finding.severity {
                aptu_core::Severity::Critical => style("CRITICAL").red().bold(),
                aptu_core::Severity::High => style("HIGH").red(),
                aptu_core::Severity::Medium => style("MEDIUM").yellow(),
                aptu_core::Severity::Low => style("LOW").dim(),
            };
            writeln!(
                w,
                "  [{}] {}:{}",
                severity_style,
                style(&finding.file_path).cyan(),
                finding.line_number
            )?;
            writeln!(w, "    {}", finding.description)?;
            if let Some(cwe) = &finding.cwe {
                writeln!(w, "    {}", style(cwe).dim())?;
            }
        }
        writeln!(w)?;
    } else {
        // Normal mode: show concise summary
        let count = findings.len();
        let critical_count = findings
            .iter()
            .filter(|f| matches!(f.severity, aptu_core::Severity::Critical))
            .count();
        let high_count = findings
            .iter()
            .filter(|f| matches!(f.severity, aptu_core::Severity::High))
            .count();

        let summary = if critical_count > 0 || high_count > 0 {
            let mut parts = vec![];
            if critical_count > 0 {
                parts.push(format!("{critical_count} CRITICAL"));
            }
            if high_count > 0 {
                parts.push(format!("{high_count} HIGH"));
            }
            format!(
                "{} finding{} ({})",
                count,
                if count == 1 { "" } else { "s" },
                parts.join(", ")
            )
        } else {
            format!("{} finding{}", count, if count == 1 { "" } else { "s" })
        };

        writeln!(
            w,
            "{}: {} {}",
            style("Security Scan").red().bold(),
            style(summary).red(),
            style("(use --verbose for details)").dim()
        )?;
        writeln!(w)?;
    }
    Ok(())
}

fn render_comments_text(
    w: &mut dyn Write,
    comments: &[aptu_core::ai::types::PrReviewComment],
) -> io::Result<()> {
    for comment in comments {
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
    Ok(())
}

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

        // Security Findings (shown early for visibility)
        if let Some(findings) = &self.security_findings {
            render_security_findings_text(w, findings, ctx)?;
            writeln!(w)?;
        }

        // Summary
        writeln!(w, "{}", style("Summary").cyan().bold())?;
        writeln!(w, "{}", self.review.summary)?;
        writeln!(w)?;

        // Disclaimer
        if let Some(disclaimer) = &self.review.disclaimer {
            writeln!(w, "{}", style("Disclaimer").yellow().bold())?;
            writeln!(w, "{disclaimer}")?;
            writeln!(w)?;
        }

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
            render_comments_text(w, &self.review.comments)?;
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
            writeln!(w, "  Prompt: {} chars", self.ai_stats.prompt_chars)?;
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

        // Security Findings
        if let Some(findings) = &self.security_findings {
            if findings.is_empty() {
                writeln!(w, "### Security Scan")?;
                writeln!(w, "No issues found")?;
                writeln!(w)?;
            } else {
                writeln!(w, "### Security Findings")?;
                for finding in findings {
                    writeln!(
                        w,
                        "- **[{}]** `{}:{}`",
                        match finding.severity {
                            aptu_core::Severity::Critical => "CRITICAL",
                            aptu_core::Severity::High => "HIGH",
                            aptu_core::Severity::Medium => "MEDIUM",
                            aptu_core::Severity::Low => "LOW",
                        },
                        finding.file_path,
                        finding.line_number
                    )?;
                    writeln!(w, "  {}", finding.description)?;
                    if let Some(cwe) = &finding.cwe {
                        writeln!(w, "  {cwe}")?;
                    }
                }
                writeln!(w)?;
            }
        }

        if let Some(disclaimer) = &self.review.disclaimer {
            writeln!(w, "### Disclaimer")?;
            writeln!(w, "> {disclaimer}")?;
            writeln!(w)?;
        }

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

impl Renderable for PrCreateResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(
            w,
            "PR #{} created: {}",
            self.pr_number,
            style(&self.url).cyan().underlined()
        )?;
        writeln!(
            w,
            "  {} -> {}",
            style(&self.branch).green(),
            style(&self.base).cyan()
        )?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "PR #{} created: {}", self.pr_number, self.url)?;
        writeln!(w, "  {} -> {}", self.branch, self.base)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_result(security_findings: Option<Vec<aptu_core::Finding>>) -> PrReviewResult {
        PrReviewResult {
            pr_title: "Test PR".to_string(),
            pr_number: 42,
            pr_url: "https://github.com/test/repo/pull/42".to_string(),
            review: aptu_core::ai::types::PrReviewResponse {
                verdict: "approve".to_string(),
                summary: "Test summary".to_string(),
                strengths: vec![],
                concerns: vec![],
                comments: vec![],
                suggestions: vec![],
                disclaimer: None,
            },
            verdict: "approve".to_string(),
            ai_stats: aptu_core::history::AiStats {
                provider: "openrouter".to_string(),
                model: "test-model".to_string(),
                input_tokens: 100,
                output_tokens: 50,
                duration_ms: 1000,
                cost_usd: None,
                fallback_provider: None,
                prompt_chars: 0,
            },
            security_findings,
            dry_run: false,
            labels: vec![],
        }
    }

    #[test]
    fn test_render_markdown_security_findings_none() {
        let result = build_test_result(None);
        let mut output = Vec::new();
        let ctx = OutputContext::from_cli(crate::cli::OutputFormat::Markdown, false);

        result.render_markdown(&mut output, &ctx).unwrap();
        let text = String::from_utf8(output).unwrap();

        assert!(!text.contains("Security Scan"));
        assert!(!text.contains("Security Findings"));
    }

    #[test]
    fn test_render_markdown_security_findings_empty() {
        let result = build_test_result(Some(vec![]));
        let mut output = Vec::new();
        let ctx = OutputContext::from_cli(crate::cli::OutputFormat::Markdown, false);

        result.render_markdown(&mut output, &ctx).unwrap();
        let text = String::from_utf8(output).unwrap();

        assert!(text.contains("### Security Scan"));
        assert!(text.contains("No issues found"));
    }

    #[test]
    fn test_render_markdown_security_findings_populated() {
        let finding = aptu_core::Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test vulnerability".to_string(),
            severity: aptu_core::Severity::High,
            confidence: aptu_core::Confidence::High,
            file_path: "src/main.rs".to_string(),
            line_number: 42,
            matched_text: "unsafe { }".to_string(),
            cwe: Some("CWE-123".to_string()),
        };

        let result = build_test_result(Some(vec![finding]));
        let mut output = Vec::new();
        let ctx = OutputContext::from_cli(crate::cli::OutputFormat::Markdown, false);

        result.render_markdown(&mut output, &ctx).unwrap();
        let text = String::from_utf8(output).unwrap();

        assert!(text.contains("### Security Findings"));
        assert!(text.contains("[HIGH]"));
        assert!(text.contains("src/main.rs:42"));
        assert!(text.contains("Test vulnerability"));
        assert!(text.contains("CWE-123"));
    }

    #[test]
    fn test_render_text_security_findings_hint_in_normal_mode() {
        let finding = aptu_core::Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test vulnerability".to_string(),
            severity: aptu_core::Severity::High,
            confidence: aptu_core::Confidence::High,
            file_path: "src/main.rs".to_string(),
            line_number: 42,
            matched_text: "unsafe { }".to_string(),
            cwe: Some("CWE-123".to_string()),
        };

        let result = build_test_result(Some(vec![finding]));
        let mut output = Vec::new();
        let ctx = OutputContext::from_cli(crate::cli::OutputFormat::Text, false);

        result.render_text(&mut output, &ctx).unwrap();
        let text = String::from_utf8(output).unwrap();

        assert!(text.contains("Security Scan"));
        assert!(text.contains("(use --verbose for details)"));
    }

    #[test]
    fn test_render_text_security_findings_no_hint_in_verbose_mode() {
        let finding = aptu_core::Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test vulnerability".to_string(),
            severity: aptu_core::Severity::High,
            confidence: aptu_core::Confidence::High,
            file_path: "src/main.rs".to_string(),
            line_number: 42,
            matched_text: "unsafe { }".to_string(),
            cwe: Some("CWE-123".to_string()),
        };

        let result = build_test_result(Some(vec![finding]));
        let mut output = Vec::new();
        let ctx = OutputContext::from_cli(crate::cli::OutputFormat::Text, true);

        result.render_text(&mut output, &ctx).unwrap();
        let text = String::from_utf8(output).unwrap();

        assert!(text.contains("Security Findings"));
        assert!(!text.contains("(use --verbose for details)"));
    }
}
