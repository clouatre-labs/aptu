// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use aptu_core::utils::parse_and_format_relative_time;
use console::style;
use std::io::{self, Write};

use crate::cli::{OutputContext, OutputFormat};
use crate::commands::types::IssuesResult;

use super::Renderable;

/// Issues output for JSON/YAML serialization.
#[derive(serde::Serialize)]
pub struct RepoIssuesOutput {
    pub repo: String,
    pub issues: Vec<aptu_core::github::graphql::IssueNode>,
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
                    aptu_core::utils::truncate(&issue.title, 50),
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

// Special handling for IssuesResult to handle no_repos_matched and custom JSON/YAML
impl IssuesResult {
    pub fn render_with_context(&self, ctx: &OutputContext) -> Result<()> {
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
            return Ok(());
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
                let json = serde_json::to_string_pretty(&output)
                    .context("Failed to serialize issues to JSON")?;
                println!("{json}");
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
                let yaml = serde_saphyr::to_string(&output)
                    .context("Failed to serialize issues to YAML")?;
                println!("{yaml}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                super::render(self, ctx)?;
            }
        }
        Ok(())
    }
}
