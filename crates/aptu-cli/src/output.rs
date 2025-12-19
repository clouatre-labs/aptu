//! Output rendering for CLI commands.
//!
//! Centralizes all output formatting logic, supporting text, JSON, and YAML formats.
//! Command handlers return data; this module handles presentation.

use aptu_core::ai::types::{IssueDetails, TriageResponse};
use aptu_core::github::graphql::IssueNode;
use aptu_core::history::ContributionStatus;
use aptu_core::triage::APTU_SIGNATURE;
use aptu_core::utils::{
    format_relative_time, parse_and_format_relative_time, truncate, truncate_with_suffix,
};
use console::style;

use crate::cli::{OutputContext, OutputFormat};
use crate::commands::types::{HistoryResult, IssuesResult, ReposResult, TriageResult};

/// Render repos result.
pub fn render_repos(result: &ReposResult, ctx: &OutputContext) {
    match ctx.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(result.repos)
                    .expect("Failed to serialize repos to JSON")
            );
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_yml::to_string(result.repos).expect("Failed to serialize repos to YAML")
            );
        }
        OutputFormat::Markdown => {
            println!("## Available Repositories\n");
            for repo in result.repos {
                println!(
                    "- **{}** ({}) - {}",
                    repo.full_name(),
                    repo.language,
                    repo.description
                );
            }
        }
        OutputFormat::Text => {
            println!();
            println!("{}", style("Available repositories:").bold());
            println!();

            for (i, repo) in result.repos.iter().enumerate() {
                let num = format!("{:>3}.", i + 1);
                let name = format!("{:<25}", repo.full_name());
                let lang = format!("{:<10}", repo.language);

                println!(
                    "  {} {} {} {}",
                    style(num).dim(),
                    style(name).cyan(),
                    style(lang).yellow(),
                    style(repo.description).dim()
                );
            }

            println!();
        }
    }
}

/// Issues output for JSON/YAML serialization.
#[derive(serde::Serialize)]
struct RepoIssuesOutput {
    repo: String,
    issues: Vec<IssueNode>,
}

/// Render issues result.
#[allow(clippy::too_many_lines)]
pub fn render_issues(result: &IssuesResult, ctx: &OutputContext) {
    // Handle "no repos matched filter" case
    if result.no_repos_matched {
        if let Some(ref filter) = result.repo_filter {
            match ctx.format {
                OutputFormat::Json | OutputFormat::Yaml => println!("[]"),
                OutputFormat::Markdown => {
                    println!("No curated repository matches '{filter}'");
                }
                OutputFormat::Text => {
                    println!(
                        "{}",
                        style(format!("No curated repository matches '{filter}'")).yellow()
                    );
                    println!("Run `aptu repos` to see available repositories.");
                }
            }
        }
        return;
    }

    match ctx.format {
        OutputFormat::Json => {
            let output: Vec<RepoIssuesOutput> = result
                .issues_by_repo
                .iter()
                .map(|(repo, issues)| RepoIssuesOutput {
                    repo: repo.clone(),
                    issues: issues.clone(),
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&output).expect("Failed to serialize issues to JSON")
            );
        }
        OutputFormat::Yaml => {
            let output: Vec<RepoIssuesOutput> = result
                .issues_by_repo
                .iter()
                .map(|(repo, issues)| RepoIssuesOutput {
                    repo: repo.clone(),
                    issues: issues.clone(),
                })
                .collect();
            println!(
                "{}",
                serde_yml::to_string(&output).expect("Failed to serialize issues to YAML")
            );
        }
        OutputFormat::Markdown => {
            if result.total_count == 0 {
                println!("No open 'good first issue' issues found.");
                return;
            }

            println!(
                "## Issues ({} across {} repositories)\n",
                result.total_count,
                result.issues_by_repo.len()
            );

            for (repo_name, issues) in &result.issues_by_repo {
                println!("### {repo_name}\n");

                for issue in issues {
                    let labels: Vec<String> = issue
                        .labels
                        .nodes
                        .iter()
                        .map(|l| format!("`{}`", l.name))
                        .collect();
                    let label_str = if labels.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", labels.join(" "))
                    };

                    println!("- **#{}** {}{}", issue.number, issue.title, label_str);
                }
                println!();
            }
        }
        OutputFormat::Text => {
            if result.total_count == 0 {
                println!(
                    "{}",
                    style("No open 'good first issue' issues found.").yellow()
                );
                return;
            }

            println!();
            println!(
                "{}",
                style(format!(
                    "Found {} issues across {} repositories:",
                    result.total_count,
                    result.issues_by_repo.len()
                ))
                .bold()
            );
            println!();

            for (repo_name, issues) in &result.issues_by_repo {
                println!("{}", style(repo_name).cyan().bold());

                for issue in issues {
                    let labels: Vec<&str> =
                        issue.labels.nodes.iter().map(|l| l.name.as_str()).collect();
                    let label_str = if labels.is_empty() {
                        String::new()
                    } else {
                        format!("[{}]", labels.join(", "))
                    };

                    let age = parse_and_format_relative_time(&issue.created_at);

                    println!(
                        "  {} {} {} {}",
                        style(format!("#{}", issue.number)).green(),
                        truncate(&issue.title, 50),
                        style(label_str).dim(),
                        style(age).dim()
                    );
                }
                println!();
            }
        }
    }
}

/// Output mode for triage rendering.
enum OutputMode {
    /// Terminal output with colors.
    Terminal,
    /// Markdown for GitHub comments.
    Markdown,
}

/// Renders a labeled list section.
fn render_list_section(
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
fn render_triage_content(
    triage: &TriageResponse,
    mode: &OutputMode,
    title: Option<(&str, u64)>,
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

    // Labels - format with backticks for markdown
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

    // Attribution (markdown only)
    if matches!(mode, OutputMode::Markdown) {
        output.push_str("---\n");
        output.push('*');
        output.push_str(APTU_SIGNATURE);
        output.push_str(" - AI-assisted OSS triage*\n");
    }

    output
}

/// Render triage result.
pub fn render_triage(result: &TriageResult, ctx: &OutputContext) {
    match ctx.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result.triage)
                    .expect("Failed to serialize triage to JSON")
            );
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_yml::to_string(&result.triage).expect("Failed to serialize triage to YAML")
            );
        }
        OutputFormat::Markdown => {
            // Include issue title/number in header for CLI markdown output
            println!(
                "## Triage for #{}: {}\n",
                result.issue_number, result.issue_title
            );
            print!(
                "{}",
                render_triage_content(&result.triage, &OutputMode::Markdown, None)
            );
        }
        OutputFormat::Text => {
            println!();
            print!(
                "{}",
                render_triage_content(
                    &result.triage,
                    &OutputMode::Terminal,
                    Some((&result.issue_title, result.issue_number))
                )
            );

            // Status messages
            if result.dry_run {
                println!("{}", style("Dry run - comment not posted.").yellow());
            } else if result.user_declined {
                println!("{}", style("Triage not posted.").yellow());
            } else if let Some(ref url) = result.comment_url {
                println!();
                println!("{}", style("Comment posted successfully!").green().bold());
                println!("  {}", style(url).cyan().underlined());
            }
        }
    }
}

/// Generates markdown content for posting to GitHub.
pub fn render_triage_markdown(triage: &TriageResponse) -> String {
    render_triage_content(triage, &OutputMode::Markdown, None)
}

/// Render fetched issue details.
pub fn render_issue(issue: &IssueDetails, ctx: &OutputContext) {
    match ctx.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(issue).expect("Failed to serialize issue to JSON")
            );
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_yml::to_string(issue).expect("Failed to serialize issue to YAML")
            );
        }
        OutputFormat::Markdown => {
            println!("## Issue #{}: {}\n", issue.number, issue.title);
            println!("**Repository:** {}/{}\n", issue.owner, issue.repo);

            println!("### Description\n");
            if issue.body.is_empty() {
                println!("[No description provided]\n");
            } else {
                let body = truncate_body(&issue.body, 1000);
                println!("{body}\n");
            }

            if !issue.labels.is_empty() {
                println!("### Labels\n");
                for label in &issue.labels {
                    println!("- `{label}`");
                }
                println!();
            }

            if !issue.comments.is_empty() {
                println!("### Comments ({})\n", issue.comments.len());
                for (i, comment) in issue.comments.iter().take(5).enumerate() {
                    println!("**{}. @{}**\n", i + 1, comment.author);
                    let body = truncate_body(&comment.body, 500);
                    println!("{body}\n");
                }
                if issue.comments.len() > 5 {
                    println!("... and {} more comments\n", issue.comments.len() - 5);
                }
            }
        }
        OutputFormat::Text => {
            println!();
            println!(
                "{}",
                style(format!("Issue #{}: {}", issue.number, issue.title))
                    .bold()
                    .underlined()
            );
            println!("{}", style(format!("{}/{}", issue.owner, issue.repo)).dim());
            println!();

            println!("{}", style("Description").cyan().bold());
            if issue.body.is_empty() {
                println!("  {}\n", style("[No description provided]").dim());
            } else {
                let body = truncate_body(&issue.body, 1000);
                println!("  {body}\n");
            }

            if !issue.labels.is_empty() {
                println!("{}", style("Labels").cyan().bold());
                for label in &issue.labels {
                    println!("  {} {}", style("-").dim(), label);
                }
                println!();
            }

            if !issue.comments.is_empty() {
                println!(
                    "{}",
                    style(format!("Comments ({})", issue.comments.len()))
                        .cyan()
                        .bold()
                );
                for (i, comment) in issue.comments.iter().take(5).enumerate() {
                    println!(
                        "  {}. {}",
                        i + 1,
                        style(format!("@{}", comment.author)).yellow()
                    );
                    let body = truncate_body(&comment.body, 500);
                    println!("     {body}");
                }
                if issue.comments.len() > 5 {
                    println!(
                        "  {}",
                        style(format!(
                            "... and {} more comments",
                            issue.comments.len() - 5
                        ))
                        .dim()
                    );
                }
                println!();
            }
        }
    }
}

/// Truncates body text to a maximum length, adding indicator if truncated.
///
/// Uses the core `truncate_with_suffix` function.
fn truncate_body(body: &str, max_len: usize) -> String {
    truncate_with_suffix(body, max_len, "... [truncated]")
}

/// Render history result.
pub fn render_history(result: &HistoryResult, ctx: &OutputContext) {
    match ctx.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result.contributions)
                    .expect("Failed to serialize history to JSON")
            );
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_yml::to_string(&result.contributions)
                    .expect("Failed to serialize history to YAML")
            );
        }
        OutputFormat::Markdown => {
            if result.contributions.is_empty() {
                println!("No contributions yet.");
                return;
            }

            println!(
                "## Contribution History ({} total)\n",
                result.contributions.len()
            );
            println!("| Repository | Issue | Action | When | Status |");
            println!("|------------|-------|--------|------|--------|");

            for contribution in &result.contributions {
                let repo = truncate(&contribution.repo, 25);
                let issue = format!("#{}", contribution.issue);
                let when = format_relative_time(&contribution.timestamp);
                let status = match contribution.status {
                    ContributionStatus::Pending => "pending",
                    ContributionStatus::Accepted => "accepted",
                    ContributionStatus::Rejected => "rejected",
                };

                println!(
                    "| {} | {} | {} | {} | {} |",
                    repo, issue, contribution.action, when, status
                );
            }
        }
        OutputFormat::Text => {
            if result.contributions.is_empty() {
                println!();
                println!("{}", style("No contributions yet.").yellow());
                println!("Run `aptu triage <url>` to get started!");
                println!();
                return;
            }

            println!();
            println!(
                "{}",
                style(format!(
                    "Contribution history ({} total):",
                    result.contributions.len()
                ))
                .bold()
            );
            println!();

            // Table header
            println!(
                "  {:<25} {:<8} {:<10} {:<15} {}",
                style("Repository").cyan(),
                style("Issue").cyan(),
                style("Action").cyan(),
                style("When").cyan(),
                style("Status").cyan()
            );
            println!("  {}", style("-".repeat(75)).dim());

            for contribution in &result.contributions {
                let repo = truncate(&contribution.repo, 25);
                let issue = format!("#{}", contribution.issue);
                let when = format_relative_time(&contribution.timestamp);
                let status = match contribution.status {
                    ContributionStatus::Pending => style("pending").yellow().to_string(),
                    ContributionStatus::Accepted => style("accepted").green().to_string(),
                    ContributionStatus::Rejected => style("rejected").red().to_string(),
                };

                println!(
                    "  {:<25} {:<8} {:<10} {:<15} {}",
                    repo,
                    style(issue).green(),
                    contribution.action,
                    style(when).dim(),
                    status
                );
            }

            println!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aptu_core::ai::types::TriageResponse;

    #[test]
    fn test_render_triage_markdown_with_all_fields() {
        let triage = TriageResponse {
            summary: "This is a bug report about a crash.".to_string(),
            suggested_labels: vec!["bug".to_string(), "crash".to_string()],
            clarifying_questions: vec!["What version are you using?".to_string()],
            potential_duplicates: vec!["#123".to_string()],
            status_note: None,
            related_issues: Vec::new(),
            contributor_guidance: None,
        };

        let comment = render_triage_markdown(&triage);

        assert!(comment.contains("## Triage Summary"));
        assert!(comment.contains("This is a bug report about a crash."));
        assert!(comment.contains("- `bug`"));
        assert!(comment.contains("- `crash`"));
        assert!(comment.contains("1. What version are you using?"));
        assert!(comment.contains("- #123"));
        assert!(comment.contains("Aptu"));
    }

    #[test]
    fn test_render_triage_markdown_with_empty_fields() {
        let triage = TriageResponse {
            summary: "Simple issue.".to_string(),
            suggested_labels: vec!["enhancement".to_string()],
            clarifying_questions: vec![],
            potential_duplicates: vec![],
            status_note: None,
            related_issues: Vec::new(),
            contributor_guidance: None,
        };

        let comment = render_triage_markdown(&triage);

        assert!(comment.contains("None needed"));
        assert!(comment.contains("None found"));
    }

    #[test]
    fn test_render_triage_markdown_with_status_note() {
        let triage = TriageResponse {
            summary: "Issue with a claimed status.".to_string(),
            suggested_labels: vec!["bug".to_string()],
            clarifying_questions: vec![],
            potential_duplicates: vec![],
            related_issues: Vec::new(),
            status_note: Some("Issue claimed by @user".to_string()),
            contributor_guidance: None,
        };

        let comment = render_triage_markdown(&triage);

        assert!(comment.contains("## Triage Summary"));
        assert!(comment.contains("Issue with a claimed status."));
        assert!(comment.contains("Status"));
        assert!(comment.contains("Issue claimed by @user"));
    }

    #[test]
    fn test_render_list_section_terminal_numbered() {
        let items = vec!["First".to_string(), "Second".to_string()];
        let output = render_list_section("Questions", &items, "None", &OutputMode::Terminal, true);

        assert!(output.contains("1. First"));
        assert!(output.contains("2. Second"));
    }

    #[test]
    fn test_render_list_section_markdown_unnumbered() {
        let items = vec!["bug".to_string(), "crash".to_string()];
        let output = render_list_section("Labels", &items, "None", &OutputMode::Markdown, false);

        assert!(output.contains("### Labels"));
        assert!(output.contains("- bug"));
        assert!(output.contains("- crash"));
    }

    #[test]
    fn test_render_list_section_empty() {
        let items: Vec<String> = vec![];
        let output = render_list_section(
            "Duplicates",
            &items,
            "None found",
            &OutputMode::Markdown,
            false,
        );

        assert!(output.contains("None found"));
    }

    #[test]
    fn test_truncate_body_short() {
        let body = "Short body";
        assert_eq!(truncate_body(body, 100), "Short body");
    }

    #[test]
    fn test_truncate_body_long() {
        let body = "This is a very long body that should be truncated because it exceeds the maximum length";
        let result = truncate_body(body, 50);
        assert!(result.ends_with("... [truncated]"));
        assert!(result.contains("This is a very long"));
    }

    #[test]
    fn test_truncate_body_exact_length() {
        let body = "Exactly fifty characters long text here now ok";
        let result = truncate_body(body, 50);
        assert_eq!(result, body);
    }

    #[test]
    fn test_render_triage_markdown_with_contributor_guidance_beginner() {
        use aptu_core::ai::types::ContributorGuidance;

        let triage = TriageResponse {
            summary: "Simple bug fix.".to_string(),
            suggested_labels: vec!["bug".to_string()],
            clarifying_questions: vec![],
            potential_duplicates: vec![],
            status_note: None,
            related_issues: Vec::new(),
            contributor_guidance: Some(ContributorGuidance {
                beginner_friendly: true,
                reasoning: "Small scope, well-defined problem statement.".to_string(),
            }),
        };

        let comment = render_triage_markdown(&triage);

        assert!(comment.contains("### Contributor Guidance"));
        assert!(comment.contains("**Beginner-friendly**"));
        assert!(comment.contains("Small scope, well-defined problem statement."));
    }

    #[test]
    fn test_render_triage_markdown_with_contributor_guidance_advanced() {
        use aptu_core::ai::types::ContributorGuidance;

        let triage = TriageResponse {
            summary: "Complex refactoring.".to_string(),
            suggested_labels: vec!["enhancement".to_string()],
            clarifying_questions: vec![],
            potential_duplicates: vec![],
            status_note: None,
            related_issues: Vec::new(),
            contributor_guidance: Some(ContributorGuidance {
                beginner_friendly: false,
                reasoning: "Requires deep knowledge of the compiler internals.".to_string(),
            }),
        };

        let comment = render_triage_markdown(&triage);

        assert!(comment.contains("### Contributor Guidance"));
        assert!(comment.contains("**Advanced**"));
        assert!(comment.contains("Requires deep knowledge of the compiler internals."));
    }

    #[test]
    fn test_render_triage_markdown_without_contributor_guidance() {
        let triage = TriageResponse {
            summary: "Standard issue.".to_string(),
            suggested_labels: vec!["bug".to_string()],
            clarifying_questions: vec![],
            potential_duplicates: vec![],
            status_note: None,
            related_issues: Vec::new(),
            contributor_guidance: None,
        };

        let comment = render_triage_markdown(&triage);

        // Should not contain contributor guidance section
        assert!(!comment.contains("### Contributor Guidance"));
    }
}
