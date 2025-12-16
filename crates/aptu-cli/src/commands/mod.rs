//! Command handlers for Aptu CLI.

pub mod auth;
pub mod history;
pub mod issue;
pub mod repo;
pub mod triage;
pub mod types;

use std::io::Write;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_complete::generate;
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::debug;

use crate::cli::{
    AuthCommand, Cli, Commands, IssueCommand, OutputContext, OutputFormat, RepoCommand,
};
use crate::output;

/// Creates a styled spinner (only if interactive).
fn maybe_spinner(ctx: &OutputContext, message: &str) -> Option<ProgressBar> {
    if ctx.is_interactive() {
        let s = ProgressBar::new_spinner();
        s.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .expect("Invalid spinner template"),
        );
        s.set_message(message.to_string());
        s.enable_steady_tick(Duration::from_millis(100));
        Some(s)
    } else {
        None
    }
}

/// Dispatch to the appropriate command handler.
pub async fn run(command: Commands, ctx: OutputContext) -> Result<()> {
    match command {
        Commands::Auth(auth_cmd) => match auth_cmd {
            AuthCommand::Login => auth::run_login().await,
            AuthCommand::Logout => auth::run_logout(),
            AuthCommand::Status => auth::run_status(),
        },

        Commands::Repo(repo_cmd) => match repo_cmd {
            RepoCommand::List => {
                let result = repo::run().await?;
                output::render_repos(&result, &ctx);
                Ok(())
            }
        },

        Commands::Issue(issue_cmd) => match issue_cmd {
            IssueCommand::List { repo } => {
                let spinner = maybe_spinner(&ctx, "Fetching issues...");
                let result = issue::run(repo).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                output::render_issues(&result, &ctx);
                Ok(())
            }
            IssueCommand::Triage { url, dry_run, yes } => {
                // Phase 1: Fetch and analyze
                let spinner = maybe_spinner(&ctx, "Fetching issue and analyzing...");
                let analyze_result = triage::analyze(&url).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }

                // Build result for rendering (before posting decision)
                let result = types::TriageResult {
                    issue_title: analyze_result.issue_title.clone(),
                    issue_number: analyze_result.issue_number,
                    triage: analyze_result.triage.clone(),
                    comment_url: None,
                    dry_run,
                    user_declined: false,
                };

                // Render triage FIRST (before asking for confirmation)
                output::render_triage(&result, &ctx);

                // Handle dry-run - already rendered, just exit
                if dry_run {
                    return Ok(());
                }

                // For non-interactive without --yes, don't post (safe default)
                if !ctx.is_interactive() && !yes {
                    return Ok(());
                }

                // Handle confirmation (now AFTER user has seen the triage)
                let should_post = if yes {
                    true
                } else {
                    let config = crate::config::load_config()?;
                    if config.ui.confirm_before_post {
                        println!();
                        Confirm::new()
                            .with_prompt("Post this triage as a comment to the issue?")
                            .default(false)
                            .interact()
                            .context("Failed to get user confirmation")?
                    } else {
                        true
                    }
                };

                if !should_post {
                    if matches!(ctx.format, OutputFormat::Text) {
                        println!("{}", style("Triage not posted.").yellow());
                    }
                    return Ok(());
                }

                // Phase 2: Post the comment
                let spinner = maybe_spinner(&ctx, "Posting comment...");
                let comment_url = triage::post(&analyze_result).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }

                // Record to history
                let contribution = crate::history::Contribution {
                    id: uuid::Uuid::new_v4(),
                    repo: format!("{}/{}", analyze_result.owner, analyze_result.repo),
                    issue: analyze_result.issue_number,
                    action: "triage".to_string(),
                    timestamp: chrono::Utc::now(),
                    comment_url: comment_url.clone(),
                    status: crate::history::ContributionStatus::Pending,
                };
                crate::history::add_contribution(contribution)?;
                debug!("Contribution recorded to history");

                // Show success
                if matches!(ctx.format, OutputFormat::Text) {
                    println!();
                    println!("{}", style("Comment posted successfully!").green().bold());
                    println!("  {}", style(&comment_url).cyan().underlined());
                }

                Ok(())
            }
        },

        Commands::History => {
            let result = history::run().await?;
            output::render_history(&result, &ctx);
            Ok(())
        }

        Commands::Completion { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut std::io::stdout());
            std::io::stdout().flush()?;
            Ok(())
        }
    }
}
