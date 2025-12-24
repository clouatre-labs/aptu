// SPDX-License-Identifier: Apache-2.0

use aptu_core::triage::APTU_SIGNATURE;
use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::types::TriageResult;

use super::{OutputMode, Renderable};

/// Renders a labeled list section.
pub fn render_list_section(
    title: &str,
    items: &[String],
    empty_msg: &str,
    mode: &OutputMode,
    numbered: bool,
) -> String {
    use std::fmt::Write;

    let mut output = String::new();

    match mode {
        OutputMode::Terminal => {
            let _ = writeln!(output, "{}", style(title).cyan().bold());
            if items.is_empty() {
                let _ = writeln!(output, "  {}", style(empty_msg).dim());
            } else if numbered {
                for (i, item) in items.iter().enumerate() {
                    let _ = writeln!(output, "  {}. {}", i + 1, item);
                }
            } else {
                for item in items {
                    let _ = writeln!(output, "  {} {}", style("-").dim(), item);
                }
            }
        }
        OutputMode::Markdown => {
            let _ = writeln!(output, "### {title}\n");
            if items.is_empty() {
                let _ = writeln!(output, "{empty_msg}");
            } else if numbered {
                for (i, item) in items.iter().enumerate() {
                    let _ = writeln!(output, "{}. {}", i + 1, item);
                }
            } else {
                for item in items {
                    let _ = writeln!(output, "- {item}");
                }
            }
        }
    }
    output.push('\n');
    output
}

/// Renders the full triage output as a string.
#[allow(clippy::too_many_lines)]
pub fn render_triage_content(
    triage: &aptu_core::ai::types::TriageResponse,
    mode: &OutputMode,
    title: Option<(&str, u64)>,
    is_maintainer: bool,
) -> String {
    use std::fmt::Write;

    let mut output = String::new();

    // Header
    match mode {
        OutputMode::Terminal => {
            if let Some((issue_title, number)) = title {
                let _ = writeln!(
                    output,
                    "{}\n",
                    style(format!("Triage for #{number}: {issue_title}"))
                        .bold()
                        .underlined()
                );
            }
            let _ = writeln!(output, "{}", style("Summary").cyan().bold());
            let _ = writeln!(output, "  {}\n", triage.summary);
        }
        OutputMode::Markdown => {
            output.push_str("## Triage Summary\n\n");
            output.push_str(&triage.summary);
            output.push_str("\n\n");
        }
    }

    // Labels - only show if maintainer
    if is_maintainer {
        let labels: Vec<String> = match mode {
            OutputMode::Terminal => triage.suggested_labels.clone(),
            OutputMode::Markdown => triage
                .suggested_labels
                .iter()
                .map(|l| format!("`{l}`"))
                .collect(),
        };
        output.push_str(&render_list_section(
            "Suggested Labels",
            &labels,
            "None",
            mode,
            false,
        ));
    }

    // Suggested Milestone - only show if maintainer
    if is_maintainer
        && let Some(milestone) = &triage.suggested_milestone
        && !milestone.is_empty()
    {
        match mode {
            OutputMode::Terminal => {
                let _ = writeln!(output, "{}", style("Suggested Milestone").cyan().bold());
                let _ = writeln!(output, "  {milestone}\n");
            }
            OutputMode::Markdown => {
                output.push_str("### Suggested Milestone\n\n");
                output.push_str(milestone);
                output.push_str("\n\n");
            }
        }
    }

    // Questions
    output.push_str(&render_list_section(
        "Clarifying Questions",
        &triage.clarifying_questions,
        "None needed",
        mode,
        true,
    ));

    // Duplicates
    output.push_str(&render_list_section(
        "Potential Duplicates",
        &triage.potential_duplicates,
        "None found",
        mode,
        false,
    ));

    // Related issues
    if !triage.related_issues.is_empty() {
        match mode {
            OutputMode::Terminal => {
                let _ = writeln!(output, "{}", style("Related Issues").cyan().bold());
                for issue in &triage.related_issues {
                    let _ = writeln!(output, "  #{} - {}", issue.number, issue.title);
                    let _ = writeln!(output, "    {}", style(&issue.reason).dim());
                }
                output.push('\n');
            }
            OutputMode::Markdown => {
                output.push_str("### Related Issues\n\n");
                for issue in &triage.related_issues {
                    let _ = writeln!(output, "- **#{}** - {}", issue.number, issue.title);
                    let _ = writeln!(output, "  > {}\n", issue.reason);
                }
            }
        }
    }

    // Status note (if present)
    if let Some(status_note) = &triage.status_note
        && !status_note.is_empty()
    {
        output.push_str(&render_list_section(
            "Status",
            std::slice::from_ref(status_note),
            "",
            mode,
            false,
        ));
    }

    // Contributor guidance (if present)
    if let Some(guidance) = &triage.contributor_guidance {
        match mode {
            OutputMode::Terminal => {
                let _ = writeln!(output, "{}", style("Contributor Guidance").cyan().bold());
                let beginner_label = if guidance.beginner_friendly {
                    style("Beginner-friendly").green()
                } else {
                    style("Advanced").yellow()
                };
                let _ = writeln!(output, "  {beginner_label}");
                let _ = writeln!(output, "  {}\n", guidance.reasoning);
            }
            OutputMode::Markdown => {
                output.push_str("### Contributor Guidance\n\n");
                let beginner_label = if guidance.beginner_friendly {
                    "**Beginner-friendly**"
                } else {
                    "**Advanced**"
                };
                let _ = writeln!(output, "{beginner_label}\n");
                let _ = writeln!(output, "{}\n", guidance.reasoning);
            }
        }
    }

    // Implementation approach (if present)
    if let Some(approach) = &triage.implementation_approach
        && !approach.is_empty()
    {
        match mode {
            OutputMode::Terminal => {
                let _ = writeln!(output, "{}", style("Implementation Approach").cyan().bold());
                let _ = writeln!(output, "  {approach}\n");
            }
            OutputMode::Markdown => {
                output.push_str("### Implementation Approach\n\n");
                let _ = writeln!(output, "{approach}\n");
            }
        }
    }

    // Attribution (markdown only)
    if matches!(mode, OutputMode::Markdown) {
        output.push_str("---\n");
        output.push('*');
        output.push_str(APTU_SIGNATURE);
        output.push_str(" - AI-assisted OSS triage*\n");
    }

    output
}

impl Renderable for TriageResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        write!(
            w,
            "{}",
            render_triage_content(
                &self.triage,
                &OutputMode::Terminal,
                Some((&self.issue_title, self.issue_number)),
                self.is_maintainer
            )
        )?;

        // Status messages
        if self.dry_run {
            writeln!(w, "{}", style("Dry run - comment not posted.").yellow())?;
        } else if self.user_declined {
            writeln!(w, "{}", style("Triage not posted.").yellow())?;
        } else if let Some(ref url) = self.comment_url {
            writeln!(w)?;
            writeln!(
                w,
                "{}",
                style("Comment posted successfully!").green().bold()
            )?;
            writeln!(w, "  {}", style(url).cyan().underlined())?;
        }

        // Show applied labels and milestone
        if !self.applied_labels.is_empty() || self.applied_milestone.is_some() {
            writeln!(w)?;
            writeln!(w, "{}", style("Applied to issue:").green())?;
            if !self.applied_labels.is_empty() {
                writeln!(w, "  Labels: {}", self.applied_labels.join(", "))?;
            }
            if let Some(ref milestone) = self.applied_milestone {
                writeln!(w, "  Milestone: {milestone}")?;
            }
        }

        // Show warnings
        if !self.apply_warnings.is_empty() {
            writeln!(w)?;
            writeln!(w, "{}", style("Warnings:").yellow())?;
            for warning in &self.apply_warnings {
                writeln!(w, "  - {warning}")?;
            }
        }
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        // Include issue title/number in header for CLI markdown output
        writeln!(
            w,
            "## Triage for #{}: {}\n",
            self.issue_number, self.issue_title
        )?;
        write!(
            w,
            "{}",
            render_triage_content(
                &self.triage,
                &OutputMode::Markdown,
                None,
                self.is_maintainer
            )
        )?;
        Ok(())
    }
}

/// Generates markdown content for posting to GitHub.
pub fn render_triage_markdown(triage: &aptu_core::ai::types::TriageResponse) -> String {
    render_triage_content(triage, &OutputMode::Markdown, None, true)
}
