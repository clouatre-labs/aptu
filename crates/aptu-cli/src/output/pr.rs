// SPDX-License-Identifier: Apache-2.0

//! PR review output rendering.

use std::io::{self, Write};

use console::style;

use crate::cli::OutputContext;
use crate::commands::types::{BulkPrReviewResult, PrReviewResult, SinglePrReviewOutcome};
use crate::output::Renderable;

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

        // Coverage (only shown when some file patches were dropped from the review context)
        if self.files_with_patch < self.files_total {
            writeln!(
                w,
                "{}",
                style(format!(
                    "Coverage: {}/{} file patches included in review context",
                    self.files_with_patch, self.files_total
                ))
                .yellow()
            )?;
            writeln!(w)?;
        }

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
            writeln!(
                w,
                "  ETU:    {:.0}",
                self.ai_stats.effective_token_units.max(0.0)
            )?;
            writeln!(w, "  Prompt: {} chars", self.ai_stats.prompt_chars)?;
            writeln!(w)?;
        }

        // Dry-run message
        if self.dry_run {
            crate::output::common::show_dry_run_message(w, "DRY RUN MODE")?;
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
            ai_stats: aptu_core::metrics::AiStats {
                provider: "openrouter".to_string(),
                model: "test-model".to_string(),
                input_tokens: 100,
                output_tokens: 50,
                duration_ms: 1000,
                cost_usd: None,
                fallback_provider: None,
                prompt_chars: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                effective_token_units: 0.0,
                trace_id: None,
            }
            .with_computed_etu(),
            security_findings,
            dry_run: false,
            labels: vec![],
            files_total: 5,
            files_with_patch: 5,
        }
    }

    fn test_finding() -> aptu_core::Finding {
        aptu_core::Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test vulnerability".to_string(),
            severity: aptu_core::Severity::High,
            confidence: aptu_core::Confidence::High,
            file_path: "src/main.rs".to_string(),
            line_number: 42,
            matched_text: "unsafe { }".to_string(),
            cwe: Some("CWE-123".to_string()),
        }
    }

    fn render_security_findings(verbose: bool) -> String {
        let result = build_test_result(Some(vec![test_finding()]));
        let mut output = Vec::new();
        let ctx = OutputContext::from_cli(crate::cli::OutputFormat::Text, verbose);

        result.render_text(&mut output, &ctx).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn test_render_text_security_findings_hint_in_normal_mode() {
        let text = render_security_findings(false);

        assert!(text.contains("Security Scan"));
        assert!(text.contains("(use --verbose for details)"));
    }

    #[test]
    fn test_render_text_security_findings_no_hint_in_verbose_mode() {
        let text = render_security_findings(true);

        assert!(text.contains("Security Findings"));
        assert!(!text.contains("(use --verbose for details)"));
    }

    #[test]
    fn test_render_text_coverage_shown_when_patches_dropped() {
        let mut result = build_test_result(None);
        result.files_total = 5;
        result.files_with_patch = 2;

        let mut output = Vec::new();
        let ctx = OutputContext::from_cli(crate::cli::OutputFormat::Text, false);

        result.render_text(&mut output, &ctx).unwrap();
        let text = String::from_utf8(output).unwrap();

        assert!(text.contains("Coverage: 2/5 file patches included in review context"));
    }

    #[test]
    fn test_render_text_coverage_hidden_when_all_patches_included() {
        let result = build_test_result(None);
        // files_total: 5, files_with_patch: 5 from build_test_result

        let mut output = Vec::new();
        let ctx = OutputContext::from_cli(crate::cli::OutputFormat::Text, false);

        result.render_text(&mut output, &ctx).unwrap();
        let text = String::from_utf8(output).unwrap();

        assert!(!text.contains("Coverage:"));
    }
}
