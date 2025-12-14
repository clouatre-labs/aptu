//! List open issues command.
//!
//! Fetches "good first issue" issues from curated repositories using
//! a single GraphQL query for optimal performance.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use tracing::{debug, info, instrument};

use crate::cli::{OutputContext, OutputFormat};
use crate::github::{auth, graphql};
use crate::repos;

/// Issues output for JSON/YAML serialization.
#[derive(Serialize)]
struct RepoIssuesOutput {
    repo: String,
    issues: Vec<graphql::IssueNode>,
}

/// List open issues suitable for contribution.
///
/// Fetches issues with "good first issue" label from all curated repositories
/// (or a specific one if `--repo` is provided).
#[instrument(skip_all, fields(repo_filter = ?repo))]
pub async fn run(repo: Option<String>, ctx: OutputContext) -> Result<()> {
    // Check authentication
    if !auth::is_authenticated() {
        anyhow::bail!("Authentication required - run `aptu auth` first");
    }

    // Get curated repos, optionally filtered
    let all_repos = repos::list();
    let repos_to_query: Vec<_> = match &repo {
        Some(filter) => {
            let filter_lower = filter.to_lowercase();
            all_repos
                .iter()
                .filter(|r| {
                    r.full_name().to_lowercase().contains(&filter_lower)
                        || r.name.to_lowercase().contains(&filter_lower)
                })
                .cloned()
                .collect()
        }
        None => all_repos.to_vec(),
    };

    if repos_to_query.is_empty() {
        if let Some(filter) = &repo {
            match ctx.format {
                OutputFormat::Json => println!("[]"),
                OutputFormat::Yaml => println!("[]"),
                OutputFormat::Text => {
                    println!(
                        "{}",
                        style(format!("No curated repository matches '{}'", filter)).yellow()
                    );
                    println!("Run `aptu repos` to see available repositories.");
                }
            }
        }
        return Ok(());
    }

    // Create authenticated client
    let client = auth::create_client().context("Failed to create GitHub client")?;

    // Show spinner while fetching (only in interactive mode)
    let spinner = if ctx.is_interactive() {
        let s = ProgressBar::new_spinner();
        s.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .expect("Invalid spinner template"),
        );
        s.set_message("Fetching issues...");
        s.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(s)
    } else {
        None
    };

    // Fetch issues via GraphQL
    let results = graphql::fetch_issues(&client, &repos_to_query).await?;

    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Count total issues
    let total_issues: usize = results.iter().map(|(_, issues)| issues.len()).sum();

    // Handle output format
    match ctx.format {
        OutputFormat::Json => {
            let output: Vec<RepoIssuesOutput> = results
                .into_iter()
                .map(|(repo, issues)| RepoIssuesOutput { repo, issues })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Yaml => {
            let output: Vec<RepoIssuesOutput> = results
                .into_iter()
                .map(|(repo, issues)| RepoIssuesOutput { repo, issues })
                .collect();
            println!("{}", serde_yml::to_string(&output)?);
        }
        OutputFormat::Text => {
            if total_issues == 0 {
                println!(
                    "{}",
                    style("No open 'good first issue' issues found.").yellow()
                );
                return Ok(());
            }

            info!(total_issues, repos = results.len(), "Found issues");

            // Display results
            println!();
            println!(
                "{}",
                style(format!(
                    "Found {} issues across {} repositories:",
                    total_issues,
                    results.len()
                ))
                .bold()
            );
            println!();

            for (repo_name, issues) in &results {
                println!("{}", style(repo_name).cyan().bold());

                for issue in issues {
                    let labels: Vec<&str> =
                        issue.labels.nodes.iter().map(|l| l.name.as_str()).collect();
                    let label_str = if labels.is_empty() {
                        String::new()
                    } else {
                        format!("[{}]", labels.join(", "))
                    };

                    // Parse and format relative time
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

    debug!("Issues listing complete");
    Ok(())
}

/// Truncates a title to a maximum length, adding ellipsis if needed.
///
/// Uses character count (not byte count) to safely handle multi-byte UTF-8.
fn truncate_title(title: &str, max_len: usize) -> String {
    if title.chars().count() <= max_len {
        title.to_string()
    } else {
        let truncated: String = title.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

/// Formats an ISO 8601 timestamp as relative time (e.g., "3 days ago").
fn format_relative_time(timestamp: &str) -> String {
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
        // Title with emoji (4-byte UTF-8 character)
        let title = "Fix emoji handling in parser";
        let result = truncate_title(title, 20);
        assert_eq!(result.chars().count(), 20);
        assert!(result.ends_with("..."));
        // Should not panic on multi-byte boundaries
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
}
