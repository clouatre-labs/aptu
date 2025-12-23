// SPDX-License-Identifier: Apache-2.0

//! Command handlers for Aptu CLI.

pub mod auth;
pub mod completion;
pub mod history;
pub mod issue;
pub mod repo;
pub mod triage;
pub mod types;

use std::time::Duration;

use anyhow::{Context, Result};
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::debug;

use crate::cli::{
    AuthCommand, Commands, CompletionCommand, IssueCommand, OutputContext, OutputFormat,
    RepoCommand,
};
use crate::output;
use aptu_core::{AppConfig, check_already_triaged};
use tracing::info;

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

/// Triage a single issue and return the result.
///
/// Returns Ok(Some(result)) if triaged successfully, Ok(None) if skipped (already triaged),
/// or Err if an error occurred.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::fn_params_excessive_bools)]
async fn triage_single_issue(
    reference: &str,
    repo_context: Option<&str>,
    dry_run: bool,
    yes: bool,
    show_issue: bool,
    force: bool,
    apply: bool,
    ctx: &OutputContext,
    config: &AppConfig,
) -> Result<Option<types::TriageResult>> {
    // Phase 1a: Fetch issue
    let spinner = maybe_spinner(ctx, "Fetching issue...");
    let issue_details = triage::fetch(reference, repo_context).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Phase 1b: Check if already triaged (unless --force)
    if force {
        info!("Forcing triage despite detection");
    } else {
        let triage_status = check_already_triaged(&issue_details);
        if triage_status.is_triaged() {
            if matches!(ctx.format, OutputFormat::Text) {
                println!("{}", style("Already triaged (skipping)").yellow());
            }
            return Ok(None);
        }
    }

    // Render issue if requested
    if show_issue {
        output::render_issue(&issue_details, ctx);
    }

    // Phase 1c: Analyze with AI
    let spinner = maybe_spinner(ctx, "Analyzing with AI...");
    let ai_response = triage::analyze(&issue_details).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Build result for rendering (before posting decision)
    let mut result = types::TriageResult {
        issue_title: issue_details.title.clone(),
        issue_number: issue_details.number,
        triage: ai_response.triage.clone(),
        comment_url: None,
        dry_run,
        user_declined: false,
        applied_labels: Vec::new(),
        applied_milestone: None,
        apply_warnings: Vec::new(),
    };

    // Render triage FIRST (before asking for confirmation)
    output::render_triage(&result, ctx);

    // Handle dry-run - already rendered, just exit
    if dry_run {
        return Ok(Some(result));
    }

    // For non-interactive without --yes, don't post (safe default)
    if !ctx.is_interactive() && !yes {
        return Ok(Some(result));
    }

    // Handle confirmation (now AFTER user has seen the triage)
    let should_post = if yes {
        true
    } else if config.ui.confirm_before_post {
        println!();
        Confirm::new()
            .with_prompt("Post this triage as a comment to the issue?")
            .default(false)
            .interact()
            .context("Failed to get user confirmation")?
    } else {
        true
    };

    if !should_post {
        if matches!(ctx.format, OutputFormat::Text) {
            println!("{}", style("Triage not posted.").yellow());
        }
        return Ok(Some(result));
    }

    // Phase 2: Post the comment
    let spinner = maybe_spinner(ctx, "Posting comment...");
    let analyze_result = triage::AnalyzeResult {
        issue_details: issue_details.clone(),
        triage: ai_response.triage.clone(),
        ai_stats: ai_response.stats.clone(),
    };
    let comment_url = triage::post(&analyze_result).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    result.comment_url = Some(comment_url.clone());

    // Phase 3: Apply labels and milestone if requested
    if apply {
        let spinner = maybe_spinner(ctx, "Applying labels and milestone...");
        let apply_result = triage::apply(&issue_details, &ai_response.triage).await?;
        if let Some(s) = spinner {
            s.finish_and_clear();
        }

        result
            .applied_labels
            .clone_from(&apply_result.applied_labels);
        result
            .applied_milestone
            .clone_from(&apply_result.applied_milestone);
        result.apply_warnings.clone_from(&apply_result.warnings);
    }

    // Record to history
    let contribution = aptu_core::history::Contribution {
        id: uuid::Uuid::new_v4(),
        repo: format!(
            "{}/{}",
            analyze_result.issue_details.owner, analyze_result.issue_details.repo
        ),
        issue: analyze_result.issue_details.number,
        action: "triage".to_string(),
        timestamp: chrono::Utc::now(),
        comment_url: comment_url.clone(),
        status: aptu_core::history::ContributionStatus::Pending,
        ai_stats: Some(ai_response.stats),
    };
    aptu_core::history::add_contribution(contribution)?;
    debug!("Contribution recorded to history");

    // Show success
    if matches!(ctx.format, OutputFormat::Text) {
        println!();
        println!("{}", style("Comment posted successfully!").green().bold());
        println!("  {}", style(&comment_url).cyan().underlined());
        if apply && (!result.applied_labels.is_empty() || result.applied_milestone.is_some()) {
            println!();
            println!("{}", style("Applied to issue:").green());
            if !result.applied_labels.is_empty() {
                println!("  Labels: {}", result.applied_labels.join(", "));
            }
            if let Some(milestone) = &result.applied_milestone {
                println!("  Milestone: {milestone}");
            }
            if !result.apply_warnings.is_empty() {
                println!();
                println!("{}", style("Warnings:").yellow());
                for warning in &result.apply_warnings {
                    println!("  - {warning}");
                }
            }
        }
    }

    Ok(Some(result))
}

/// Dispatch to the appropriate command handler.
#[allow(clippy::too_many_lines)]
pub async fn run(command: Commands, ctx: OutputContext, config: &AppConfig) -> Result<()> {
    match command {
        Commands::Auth(auth_cmd) => match auth_cmd {
            AuthCommand::Login => auth::run_login().await,
            AuthCommand::Logout => auth::run_logout(),
            AuthCommand::Status => {
                auth::run_status();
                Ok(())
            }
        },

        Commands::Repo(repo_cmd) => match repo_cmd {
            RepoCommand::List => {
                let result = repo::run();
                output::render_repos(&result, &ctx);
                Ok(())
            }
        },

        Commands::Issue(issue_cmd) => match issue_cmd {
            IssueCommand::List { repo, no_cache } => {
                let spinner = maybe_spinner(&ctx, "Fetching issues...");
                let result = issue::run(repo, no_cache).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                output::render_issues(&result, &ctx);
                Ok(())
            }
            IssueCommand::Triage {
                references,
                repo,
                untriaged,
                since,
                delay,
                dry_run,
                yes,
                show_issue,
                force,
                apply,
            } => {
                // Determine repo context: --repo flag > default_repo config
                let repo_context = repo.as_deref().or(config.user.default_repo.as_deref());

                // Resolve issue numbers from references or --untriaged flag
                let issue_refs = if untriaged {
                    // Fetch untriaged issues from the specified repo
                    if repo_context.is_none() {
                        anyhow::bail!("--untriaged requires --repo or default_repo config");
                    }
                    let (owner, repo_name) = repo_context
                        .unwrap()
                        .split_once('/')
                        .context("Invalid repo format, expected 'owner/repo'")?;

                    let spinner = maybe_spinner(&ctx, "Fetching untriaged issues...");
                    let client = aptu_core::github::auth::create_client()
                        .context("Failed to create GitHub client")?;
                    let untriaged_issues = aptu_core::github::graphql::fetch_untriaged_issues(
                        &client,
                        owner,
                        repo_name,
                        since.as_deref(),
                    )
                    .await?;
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }

                    untriaged_issues
                        .into_iter()
                        .map(|issue| format!("{}#{}", repo_context.unwrap(), issue.number))
                        .collect()
                } else {
                    references
                };

                if issue_refs.is_empty() {
                    if matches!(ctx.format, OutputFormat::Text) {
                        println!("{}", style("No issues to triage.").yellow());
                    }
                    return Ok(());
                }

                // Bulk triage loop
                let mut bulk_result = types::BulkTriageResult {
                    succeeded: 0,
                    failed: 0,
                    skipped: 0,
                    outcomes: Vec::new(),
                };

                for (idx, issue_ref) in issue_refs.iter().enumerate() {
                    // Progress output
                    if matches!(ctx.format, OutputFormat::Text) {
                        println!(
                            "\n[{}/{}] Triaging {}",
                            idx + 1,
                            issue_refs.len(),
                            style(issue_ref).cyan()
                        );
                    }

                    // Triage single issue
                    match triage_single_issue(
                        issue_ref,
                        repo_context,
                        dry_run,
                        yes,
                        show_issue,
                        force,
                        apply,
                        &ctx,
                        config,
                    )
                    .await
                    {
                        Ok(Some(result)) => {
                            bulk_result.succeeded += 1;
                            bulk_result.outcomes.push((
                                issue_ref.clone(),
                                types::SingleTriageOutcome::Success(Box::new(result)),
                            ));
                        }
                        Ok(None) => {
                            bulk_result.skipped += 1;
                            bulk_result.outcomes.push((
                                issue_ref.clone(),
                                types::SingleTriageOutcome::Skipped("Already triaged".to_string()),
                            ));
                        }
                        Err(e) => {
                            bulk_result.failed += 1;
                            bulk_result.outcomes.push((
                                issue_ref.clone(),
                                types::SingleTriageOutcome::Failed(e.to_string()),
                            ));
                            if matches!(ctx.format, OutputFormat::Text) {
                                println!("  {}", style(format!("Error: {e}")).red());
                            }
                        }
                    }

                    // Rate limiting between requests
                    if idx < issue_refs.len() - 1 {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                }

                // Render bulk summary
                output::render_bulk_triage_summary(&bulk_result, &ctx);

                Ok(())
            }
        },

        Commands::History => {
            let result = history::run()?;
            output::render_history(&result, &ctx);
            Ok(())
        }

        Commands::Completion(completion_cmd) => match completion_cmd {
            CompletionCommand::Generate { shell } => completion::run_generate(shell),
            CompletionCommand::Install { shell, dry_run } => {
                completion::run_install(shell, dry_run)
            }
        },
    }
}
