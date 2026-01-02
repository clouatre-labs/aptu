// SPDX-License-Identifier: Apache-2.0

use aptu_core::triage::render_triage_markdown;
use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::types::TriageResult;

use super::Renderable;

/// Renders a labeled list section for terminal output.
pub fn render_list_section(
    title: &str,
    items: &[String],
    empty_msg: &str,
    numbered: bool,
) -> String {
    use std::fmt::Write;

    let mut output = String::new();

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
    output.push('\n');
    output
}

/// Renders the full triage output as a string for terminal display.
#[allow(clippy::too_many_lines)]
pub fn render_triage_content(
    triage: &aptu_core::ai::types::TriageResponse,
    title: Option<(&str, u64)>,
    is_maintainer: bool,
) -> String {
    use std::fmt::Write;

    let mut output = String::new();

    // Header
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

    // Labels - only show if maintainer
    if is_maintainer {
        output.push_str(&render_list_section(
            "Suggested Labels",
            &triage.suggested_labels,
            "None",
            false,
        ));
    }

    // Suggested Milestone - only show if maintainer
    if is_maintainer
        && let Some(milestone) = &triage.suggested_milestone
        && !milestone.is_empty()
    {
        let _ = writeln!(output, "{}", style("Suggested Milestone").cyan().bold());
        let _ = writeln!(output, "  {milestone}\n");
    }

    // Questions
    output.push_str(&render_list_section(
        "Clarifying Questions",
        &triage.clarifying_questions,
        "None needed",
        true,
    ));

    // Duplicates
    output.push_str(&render_list_section(
        "Potential Duplicates",
        &triage.potential_duplicates,
        "None found",
        false,
    ));

    // Related issues
    if !triage.related_issues.is_empty() {
        let _ = writeln!(output, "{}", style("Related Issues").cyan().bold());
        for issue in &triage.related_issues {
            let _ = writeln!(output, "  #{} - {}", issue.number, issue.title);
            let _ = writeln!(output, "    {}", style(&issue.reason).dim());
        }
        output.push('\n');
    }

    // Status note (if present)
    if let Some(status_note) = &triage.status_note
        && !status_note.is_empty()
    {
        output.push_str(&render_list_section(
            "Status",
            std::slice::from_ref(status_note),
            "",
            false,
        ));
    }

    // Contributor guidance (if present)
    if let Some(guidance) = &triage.contributor_guidance {
        let _ = writeln!(output, "{}", style("Contributor Guidance").cyan().bold());
        let beginner_label = if guidance.beginner_friendly {
            style("Beginner-friendly").green()
        } else {
            style("Advanced").yellow()
        };
        let _ = writeln!(output, "  {beginner_label}");
        let _ = writeln!(output, "  {}\n", guidance.reasoning);
    }

    // Implementation approach (if present)
    if let Some(approach) = &triage.implementation_approach
        && !approach.is_empty()
    {
        let _ = writeln!(output, "{}", style("Implementation Approach").cyan().bold());
        for line in approach.lines() {
            let _ = writeln!(output, "  {line}");
        }
        output.push('\n');
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
                Some((&self.issue_title, self.issue_number)),
                self.is_maintainer
            )
        )?;

        // Status messages
        if self.dry_run {
            crate::output::common::show_dry_run_message(w, "Dry run - comment not posted.")?;
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
        write!(w, "{}", render_triage_markdown(&self.triage))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_triage_content_multiline_approach_indentation() {
        let triage = aptu_core::ai::types::TriageResponse {
            summary: "Test summary".to_string(),
            implementation_approach: Some("First line\nSecond line\nThird line".to_string()),
            ..Default::default()
        };

        let output = render_triage_content(&triage, None, false);

        // Verify each line of implementation_approach is prefixed with 2 spaces
        let lines: Vec<&str> = output.lines().collect();

        // Find the implementation approach section
        let mut found_approach = false;
        for (i, line) in lines.iter().enumerate() {
            if line.contains("Implementation Approach") {
                found_approach = true;
                // Next lines should be the approach content with 2-space indent
                if i + 1 < lines.len() {
                    assert!(
                        lines[i + 1].starts_with("  First line"),
                        "First line should be indented with 2 spaces, got: '{}'",
                        lines[i + 1]
                    );
                }
                if i + 2 < lines.len() {
                    assert!(
                        lines[i + 2].starts_with("  Second line"),
                        "Second line should be indented with 2 spaces, got: '{}'",
                        lines[i + 2]
                    );
                }
                if i + 3 < lines.len() {
                    assert!(
                        lines[i + 3].starts_with("  Third line"),
                        "Third line should be indented with 2 spaces, got: '{}'",
                        lines[i + 3]
                    );
                }
                break;
            }
        }

        assert!(
            found_approach,
            "Implementation Approach section not found in output"
        );
    }
}
