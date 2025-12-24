// SPDX-License-Identifier: Apache-2.0

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
use serde::Serialize;
use std::io::{self, Write};

use crate::cli::{OutputContext, OutputFormat};
use crate::commands::types::{
    AuthStatusResult, BulkTriageResult, CreateResult, HistoryResult, IssuesResult, ReposResult,
    TriageResult,
};

/// Trait for types that can be rendered in multiple output formats.
pub trait Renderable: Serialize {
    /// Render as human-readable text to the given writer.
    fn render_text(&self, w: &mut dyn Write, ctx: &OutputContext) -> io::Result<()>;

    /// Render as markdown. Defaults to text rendering.
    fn render_markdown(&self, w: &mut dyn Write, ctx: &OutputContext) -> io::Result<()> {
        self.render_text(w, ctx)
    }
}

/// Generic render function - handles JSON/YAML via serde, delegates text/markdown to trait.
pub fn render<T: Renderable>(result: &T, ctx: &OutputContext) {
    match ctx.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(result).expect("Failed to serialize to JSON")
            );
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_yml::to_string(result).expect("Failed to serialize to YAML")
            );
        }
        OutputFormat::Markdown => {
            result
                .render_markdown(&mut io::stdout(), ctx)
                .expect("Failed to render markdown");
        }
        OutputFormat::Text => {
            result
                .render_text(&mut io::stdout(), ctx)
                .expect("Failed to render text");
        }
    }
}

impl Renderable for AuthStatusResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        if self.authenticated {
            writeln!(w, "{} Authenticated with GitHub", style("*").green().bold())?;
            if let Some(ref method) = self.method {
                writeln!(w, "  Method: {}", style(method.to_string()).cyan())?;
            }
            if let Some(ref username) = self.username {
                writeln!(w, "  Username: {}", style(username).cyan())?;
            }
        } else {
            writeln!(
                w,
                "{} Not authenticated. Run {} to authenticate.",
                style("!").yellow().bold(),
                style("aptu auth login").cyan()
            )?;
        }
        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## Authentication Status\n")?;
        if self.authenticated {
            writeln!(w, "**Status:** Authenticated")?;
            if let Some(ref method) = self.method {
                writeln!(w, "**Method:** {method}")?;
            }
            if let Some(ref username) = self.username {
                writeln!(w, "**Username:** {username}")?;
            }
        } else {
            writeln!(w, "**Status:** Not authenticated")?;
        }
        Ok(())
    }
}

impl Renderable for ReposResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", style("Available repositories:").bold())?;
        writeln!(w)?;

        for (i, repo) in self.repos.iter().enumerate() {
            let num = format!("{:>3}.", i + 1);
            let name = format!("{:<25}", repo.full_name());
            let lang = format!("{:<10}", repo.language);

            writeln!(
                w,
                "  {} {} {} {}",
                style(num).dim(),
                style(name).cyan(),
                style(lang).yellow(),
                style(&repo.description).dim()
            )?;
        }

        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## Available Repositories\n")?;
        for repo in &self.repos {
            writeln!(
                w,
                "- **{}** ({}) - {}",
                repo.full_name(),
                repo.language,
                &repo.description
            )?;
        }
        Ok(())
    }
}

impl Renderable for IssuesResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        if self.total_count == 0 {
            writeln!(
                w,
                "{}",
                style("No open 'good first issue' issues found.").yellow()
            )?;
            return Ok(());
        }

        writeln!(w)?;
        writeln!(
            w,
            "{}",
            style(format!(
                "Found {} issues across {} repositories:",
                self.total_count,
                self.issues_by_repo.len()
            ))
            .bold()
        )?;
        writeln!(w)?;

        for (repo_name, issues) in &self.issues_by_repo {
            writeln!(w, "{}", style(repo_name).cyan().bold())?;

            for issue in issues {
                let labels: Vec<&str> =
                    issue.labels.nodes.iter().map(|l| l.name.as_str()).collect();
                let label_str = if labels.is_empty() {
                    String::new()
                } else {
                    format!("[{}]", labels.join(", "))
                };

                let age = parse_and_format_relative_time(&issue.created_at);

                writeln!(
                    w,
                    "  {} {} {} {}",
                    style(format!("#{}", issue.number)).green(),
                    truncate(&issue.title, 50),
                    style(label_str).dim(),
                    style(age).dim()
                )?;
            }
            writeln!(w)?;
        }
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        if self.total_count == 0 {
            writeln!(w, "No open 'good first issue' issues found.")?;
            return Ok(());
        }

        writeln!(
            w,
            "## Issues ({} across {} repositories)\n",
            self.total_count,
            self.issues_by_repo.len()
        )?;

        for (repo_name, issues) in &self.issues_by_repo {
            writeln!(w, "### {repo_name}\n")?;

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

                writeln!(w, "- **#{}** {}{}", issue.number, issue.title, label_str)?;
            }
            writeln!(w)?;
        }
        Ok(())
    }
}

// Special handling for ReposResult to maintain backward compatibility with JSON output
impl ReposResult {
    pub fn render_with_context(&self, ctx: &OutputContext) {
        match ctx.format {
            OutputFormat::Json => {
                // Output just the repos array for backward compatibility
                println!(
                    "{}",
                    serde_json::to_string_pretty(&self.repos)
                        .expect("Failed to serialize repos to JSON")
                );
            }
            OutputFormat::Yaml => {
                // Output just the repos array for backward compatibility
                println!(
                    "{}",
                    serde_yml::to_string(&self.repos).expect("Failed to serialize repos to YAML")
                );
            }
            _ => {
                // Use the trait implementation for text/markdown
                render(self, ctx);
            }
        }
    }
}

/// Issues output for JSON/YAML serialization.
#[derive(serde::Serialize)]
struct RepoIssuesOutput {
    repo: String,
    issues: Vec<IssueNode>,
}

// Special handling for IssuesResult to handle no_repos_matched and custom JSON/YAML
impl IssuesResult {
    pub fn render_with_context(&self, ctx: &OutputContext) {
        // Handle "no repos matched filter" case
        if self.no_repos_matched {
            if let Some(ref filter) = self.repo_filter {
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
                let output: Vec<RepoIssuesOutput> = self
                    .issues_by_repo
                    .iter()
                    .map(|(repo, issues)| RepoIssuesOutput {
                        repo: repo.clone(),
                        issues: issues.clone(),
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output)
                        .expect("Failed to serialize issues to JSON")
                );
            }
            OutputFormat::Yaml => {
                let output: Vec<RepoIssuesOutput> = self
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
            _ => {
                // Use the trait implementation for text/markdown
                render(self, ctx);
            }
        }
    }
}

/// Output mode for triage rendering.
pub enum OutputMode {
    /// Terminal output with colors.
    Terminal,
    /// Markdown for GitHub comments.
    Markdown,
}

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
fn render_triage_content(
    triage: &TriageResponse,
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

// Special handling for TriageResult to serialize just the triage field for JSON/YAML
impl TriageResult {
    #[allow(dead_code)]
    pub fn render_with_context(&self, ctx: &OutputContext) {
        match ctx.format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&self.triage)
                        .expect("Failed to serialize triage to JSON")
                );
            }
            OutputFormat::Yaml => {
                println!(
                    "{}",
                    serde_yml::to_string(&self.triage).expect("Failed to serialize triage to YAML")
                );
            }
            _ => {
                // Use the trait implementation for text/markdown
                render(self, ctx);
            }
        }
    }
}

/// Generates markdown content for posting to GitHub.
pub fn render_triage_markdown(triage: &TriageResponse) -> String {
    render_triage_content(triage, &OutputMode::Markdown, None, true)
}

/// Render fetched issue details.
#[allow(dead_code)]
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
#[allow(dead_code)]
fn truncate_body(body: &str, max_len: usize) -> String {
    truncate_with_suffix(body, max_len, "... [truncated]")
}

impl Renderable for HistoryResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        if self.contributions.is_empty() {
            writeln!(w)?;
            writeln!(w, "{}", style("No contributions yet.").yellow())?;
            writeln!(w, "Run `aptu triage <url>` to get started!")?;
            writeln!(w)?;
            return Ok(());
        }

        writeln!(w)?;
        writeln!(
            w,
            "{}",
            style(format!(
                "Contribution history ({} total):",
                self.contributions.len()
            ))
            .bold()
        )?;
        writeln!(w)?;

        // Table header
        writeln!(
            w,
            "  {:<25} {:<8} {:<10} {:<15} {}",
            style("Repository").cyan(),
            style("Issue").cyan(),
            style("Action").cyan(),
            style("When").cyan(),
            style("Status").cyan()
        )?;
        writeln!(w, "  {}", style("-".repeat(75)).dim())?;

        for contribution in &self.contributions {
            let repo = truncate(&contribution.repo, 25);
            let issue = format!("#{}", contribution.issue);
            let when = format_relative_time(&contribution.timestamp);
            let status = match contribution.status {
                ContributionStatus::Pending => style("pending").yellow().to_string(),
                ContributionStatus::Accepted => style("accepted").green().to_string(),
                ContributionStatus::Rejected => style("rejected").red().to_string(),
            };
            writeln!(
                w,
                "  {:<25} {:<8} {:<10} {:<15} {}",
                repo,
                style(issue).green(),
                contribution.action,
                style(when).dim(),
                status
            )?;
        }

        // AI stats
        let total_tokens = self.history_data.total_tokens();
        let total_cost = self.history_data.total_cost();
        let avg_tokens = self.history_data.avg_tokens_per_triage();

        if total_tokens > 0 {
            writeln!(w)?;
            writeln!(w, "  {}", style("AI Usage Summary").cyan().bold())?;
            writeln!(w, "  {}", style("-".repeat(75)).dim())?;
            writeln!(
                w,
                "  Total tokens: {}",
                style(total_tokens.to_string()).green()
            )?;
            writeln!(
                w,
                "  Total cost: {}",
                style(format!("${total_cost:.4}")).green()
            )?;
            writeln!(
                w,
                "  Average tokens per triage: {}",
                style(format!("{avg_tokens:.0}")).green()
            )?;
        }
        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        if self.contributions.is_empty() {
            writeln!(w, "No contributions yet.")?;
            return Ok(());
        }

        writeln!(
            w,
            "## Contribution History ({} total)\n",
            self.contributions.len()
        )?;
        writeln!(w, "| Repository | Issue | Action | When | Status |")?;
        writeln!(w, "|------------|-------|--------|------|--------|")?;

        for contribution in &self.contributions {
            let repo = truncate(&contribution.repo, 25);
            let issue = format!("#{}", contribution.issue);
            let when = format_relative_time(&contribution.timestamp);
            let status = match contribution.status {
                ContributionStatus::Pending => "pending",
                ContributionStatus::Accepted => "accepted",
                ContributionStatus::Rejected => "rejected",
            };
            writeln!(
                w,
                "| {repo} | {issue} | {} | {when} | {status} |",
                contribution.action
            )?;
        }

        // AI stats
        let total_tokens = self.history_data.total_tokens();
        let total_cost = self.history_data.total_cost();
        let avg_tokens = self.history_data.avg_tokens_per_triage();

        if total_tokens > 0 {
            writeln!(w)?;
            writeln!(w, "### AI Usage Summary")?;
            writeln!(w)?;
            writeln!(w, "- Total tokens: {total_tokens}")?;
            writeln!(w, "- Total cost: ${total_cost:.4}")?;
            writeln!(w, "- Average tokens per triage: {avg_tokens:.0}")?;
        }
        Ok(())
    }
}

// Special handling for HistoryResult to serialize just contributions for JSON/YAML
impl HistoryResult {
    #[allow(dead_code)]
    pub fn render_with_context(&self, ctx: &OutputContext) {
        match ctx.format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&self.contributions)
                        .expect("Failed to serialize history to JSON")
                );
            }
            OutputFormat::Yaml => {
                println!(
                    "{}",
                    serde_yml::to_string(&self.contributions)
                        .expect("Failed to serialize history to YAML")
                );
            }
            _ => {
                // Use the trait implementation for text/markdown
                render(self, ctx);
            }
        }
    }
}

impl Renderable for BulkTriageResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
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
        Ok(())
    }
}

// Special handling for BulkTriageResult to use custom JSON/YAML structure
impl BulkTriageResult {
    #[allow(dead_code)]
    pub fn render_with_context(&self, ctx: &OutputContext) {
        match ctx.format {
            OutputFormat::Json => {
                let summary = serde_json::json!({
                    "succeeded": self.succeeded,
                    "failed": self.failed,
                    "skipped": self.skipped,
                    "total": self.succeeded + self.failed + self.skipped,
                    "results": self.outcomes.iter().map(|(repo, outcome)| {
                        serde_json::json!({
                            "repository": repo,
                            "outcome": outcome,
                        })
                    }).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).expect("Failed to serialize summary")
                );
            }
            OutputFormat::Yaml => {
                let summary = serde_json::json!({
                    "succeeded": self.succeeded,
                    "failed": self.failed,
                    "skipped": self.skipped,
                    "total": self.succeeded + self.failed + self.skipped,
                    "results": self.outcomes.iter().map(|(repo, outcome)| {
                        serde_json::json!({
                            "repository": repo,
                            "outcome": outcome,
                        })
                    }).collect::<Vec<_>>(),
                });
                let yaml = serde_yml::to_string(&summary).expect("Failed to serialize summary");
                println!("{yaml}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                render(self, ctx);
            }
        }
    }
}

impl Renderable for CreateResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        if self.dry_run {
            writeln!(
                w,
                "{}",
                style("DRY RUN - Issue not created").yellow().bold()
            )?;
        } else {
            writeln!(w, "{}", style("Issue Created Successfully").green().bold())?;
            writeln!(w, "  Number: {}", style(self.issue_number).cyan())?;
            writeln!(w, "  URL: {}", style(&self.issue_url).cyan().underlined())?;
        }
        writeln!(w)?;
        writeln!(w, "{}", style("Title").bold())?;
        writeln!(w, "  {}", self.title)?;
        writeln!(w)?;
        writeln!(w, "{}", style("Body").bold())?;
        for line in self.body.lines() {
            writeln!(w, "  {line}")?;
        }
        if !self.suggested_labels.is_empty() {
            writeln!(w)?;
            writeln!(w, "{}", style("Suggested Labels").bold())?;
            for label in &self.suggested_labels {
                writeln!(w, "  - {}", style(label).yellow())?;
            }
        }
        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## Issue Created\n")?;
        writeln!(w, "**Title:** {}\n", self.title)?;
        writeln!(w, "**URL:** {}\n", self.issue_url)?;
        if !self.suggested_labels.is_empty() {
            writeln!(
                w,
                "**Suggested Labels:** {}\n",
                self.suggested_labels.join(", ")
            )?;
        }
        writeln!(w, "### Description\n")?;
        writeln!(w, "{}", self.body)?;
        Ok(())
    }
}

// Special handling for CreateResult to use custom JSON/YAML structure
impl CreateResult {
    #[allow(dead_code)]
    pub fn render_with_context(&self, ctx: &OutputContext) {
        match ctx.format {
            OutputFormat::Json => {
                let json = serde_json::json!({
                    "issue_url": self.issue_url,
                    "issue_number": self.issue_number,
                    "title": self.title,
                    "body": self.body,
                    "suggested_labels": self.suggested_labels,
                    "dry_run": self.dry_run,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json).expect("Failed to serialize create result")
                );
            }
            OutputFormat::Yaml => {
                let yaml = serde_yml::to_string(&serde_json::json!({
                    "issue_url": self.issue_url,
                    "issue_number": self.issue_number,
                    "title": self.title,
                    "body": self.body,
                    "suggested_labels": self.suggested_labels,
                    "dry_run": self.dry_run,
                }))
                .expect("Failed to serialize create result");
                println!("{yaml}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                render(self, ctx);
            }
        }
    }
}
