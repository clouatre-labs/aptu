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
    #[allow(clippy::too_many_lines)]
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
            ai_stats: aptu_core::history::AiStats {
                model: "test-model".to_string(),
                input_tokens: 100,
                output_tokens: 50,
                duration_ms: 1000,
                cost_usd: None,
                fallback_provider: None,
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
