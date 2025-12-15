//! Output rendering for CLI commands.
//!
//! Centralizes all output formatting logic, supporting text, JSON, and YAML formats.
//! Command handlers return data; this module handles presentation.

use chrono::{DateTime, Utc};
use console::style;

use crate::ai::types::TriageResponse;
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
    issues: Vec<crate::github::graphql::IssueNode>,
}

/// Render issues result.
pub fn render_issues(result: &IssuesResult, ctx: &OutputContext) {
    // Handle "no repos matched filter" case
    if result.no_repos_matched {
        if let Some(ref filter) = result.repo_filter {
            match ctx.format {
                OutputFormat::Json | OutputFormat::Yaml => println!("[]"),
                OutputFormat::Text => {
                    println!(
                        "{}",
                        style(format!("No curated repository matches '{}'", filter)).yellow()
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

                    let age = format_relative_time(&issue.created_at);

                    println!(
                        "  {} {} {} {}",
                        style(format!("#{}", issue.number)).green(),
                        truncate_title(&issue.title, 50),
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
    let mut output = String::new();

    match mode {
        OutputMode::Terminal => {
            output.push_str(&format!("{}\n", style(title).cyan().bold()));
            if items.is_empty() {
                output.push_str(&format!("  {}\n", style(empty_msg).dim()));
            } else if numbered {
                for (i, item) in items.iter().enumerate() {
                    output.push_str(&format!("  {}. {}\n", i + 1, item));
                }
            } else {
                for item in items {
                    output.push_str(&format!("  {} {}\n", style("-").dim(), item));
                }
            }
        }
        OutputMode::Markdown => {
            output.push_str(&format!("### {}\n\n", title));
            if items.is_empty() {
                output.push_str(&format!("{}\n", empty_msg));
            } else if numbered {
                for (i, item) in items.iter().enumerate() {
                    output.push_str(&format!("{}. {}\n", i + 1, item));
                }
            } else {
                for item in items {
                    output.push_str(&format!("- {}\n", item));
                }
            }
        }
    }
    output.push('\n');
    output
}

/// Renders the full triage output as a string.
fn render_triage_content(
    triage: &TriageResponse,
    mode: &OutputMode,
    title: Option<(&str, u64)>,
) -> String {
    let mut output = String::new();

    // Header
    match mode {
        OutputMode::Terminal => {
            if let Some((issue_title, number)) = title {
                output.push_str(&format!(
                    "{}\n\n",
                    style(format!("Triage for #{}: {}", number, issue_title))
                        .bold()
                        .underlined()
                ));
            }
            output.push_str(&format!("{}\n", style("Summary").cyan().bold()));
            output.push_str(&format!("  {}\n\n", triage.summary));
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
            .map(|l| format!("`{}`", l))
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

    // Attribution (markdown only)
    if matches!(mode, OutputMode::Markdown) {
        output.push_str("---\n");
        output.push_str(
            "*Generated by [Aptu](https://github.com/clouatre-labs/project-aptu) - AI-assisted OSS triage*\n",
        );
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
                let repo = truncate_title(&contribution.repo, 25);
                let issue = format!("#{}", contribution.issue);
                let when = format_relative_time_dt(&contribution.timestamp);
                let status = match contribution.status {
                    crate::history::ContributionStatus::Pending => {
                        style("pending").yellow().to_string()
                    }
                    crate::history::ContributionStatus::Accepted => {
                        style("accepted").green().to_string()
                    }
                    crate::history::ContributionStatus::Rejected => {
                        style("rejected").red().to_string()
                    }
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

/// Formats a DateTime<Utc> as relative time (e.g., "3 days ago").
fn format_relative_time_dt(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    if duration.num_days() > 30 {
        let months = duration.num_days() / 30;
        if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{} months ago", months)
        }
    } else if duration.num_days() > 0 {
        let days = duration.num_days();
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if duration.num_hours() > 0 {
        let hours = duration.num_hours();
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        }
    } else {
        "just now".to_string()
    }
}

/// Truncates a title to a maximum length, adding ellipsis if needed.
///
/// Uses character count (not byte count) to safely handle multi-byte UTF-8.
pub fn truncate_title(title: &str, max_len: usize) -> String {
    if title.chars().count() <= max_len {
        title.to_string()
    } else {
        let truncated: String = title.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

/// Formats an ISO 8601 timestamp as relative time (e.g., "3 days ago").
pub fn format_relative_time(timestamp: &str) -> String {
    let parsed: Result<DateTime<Utc>, _> = timestamp.parse();
    match parsed {
        Ok(dt) => {
            let now = Utc::now();
            let duration = now.signed_duration_since(dt);

            if duration.num_days() > 30 {
                let months = duration.num_days() / 30;
                if months == 1 {
                    "1 month ago".to_string()
                } else {
                    format!("{} months ago", months)
                }
            } else if duration.num_days() > 0 {
                let days = duration.num_days();
                if days == 1 {
                    "1 day ago".to_string()
                } else {
                    format!("{} days ago", days)
                }
            } else if duration.num_hours() > 0 {
                let hours = duration.num_hours();
                if hours == 1 {
                    "1 hour ago".to_string()
                } else {
                    format!("{} hours ago", hours)
                }
            } else {
                "just now".to_string()
            }
        }
        Err(_) => timestamp.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::TriageResponse;

    #[test]
    fn truncate_short_title() {
        assert_eq!(truncate_title("Short title", 50), "Short title");
    }

    #[test]
    fn truncate_long_title() {
        let long =
            "This is a very long title that should be truncated because it exceeds the limit";
        let result = truncate_title(long, 30);
        assert_eq!(result.chars().count(), 30);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_utf8_multibyte() {
        let title = "Fix emoji handling in parser";
        let result = truncate_title(title, 20);
        assert_eq!(result.chars().count(), 20);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn relative_time_days() {
        use chrono::Duration;
        let three_days_ago = (Utc::now() - Duration::days(3)).to_rfc3339();
        assert_eq!(format_relative_time(&three_days_ago), "3 days ago");
    }

    #[test]
    fn relative_time_one_day() {
        use chrono::Duration;
        let one_day_ago = (Utc::now() - Duration::days(1)).to_rfc3339();
        assert_eq!(format_relative_time(&one_day_ago), "1 day ago");
    }

    #[test]
    fn relative_time_hours() {
        use chrono::Duration;
        let five_hours_ago = (Utc::now() - Duration::hours(5)).to_rfc3339();
        assert_eq!(format_relative_time(&five_hours_ago), "5 hours ago");
    }

    #[test]
    fn relative_time_just_now() {
        let now = Utc::now().to_rfc3339();
        assert_eq!(format_relative_time(&now), "just now");
    }

    #[test]
    fn relative_time_months() {
        use chrono::Duration;
        let two_months_ago = (Utc::now() - Duration::days(65)).to_rfc3339();
        assert_eq!(format_relative_time(&two_months_ago), "2 months ago");
    }

    #[test]
    fn test_render_triage_markdown_with_all_fields() {
        let triage = TriageResponse {
            summary: "This is a bug report about a crash.".to_string(),
            suggested_labels: vec!["bug".to_string(), "crash".to_string()],
            clarifying_questions: vec!["What version are you using?".to_string()],
            potential_duplicates: vec!["#123".to_string()],
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
        };

        let comment = render_triage_markdown(&triage);

        assert!(comment.contains("None needed"));
        assert!(comment.contains("None found"));
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
}
