// SPDX-License-Identifier: Apache-2.0

//! Command handlers for Aptu CLI.

pub mod auth;
pub mod completion;
pub mod create;
pub mod history;
pub mod issue;
pub mod pr;
pub mod repo;
pub mod triage;
pub mod types;

use std::time::Duration;

use anyhow::{Context, Result};
use console::style;
use dialoguer::Confirm;
use futures::{StreamExt, stream};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::debug;

use crate::cli::{
    AuthCommand, Commands, CompletionCommand, IssueCommand, OutputContext, OutputFormat, PrCommand,
    RepoCommand,
};
use crate::output;
use aptu_core::{AppConfig, check_already_triaged};

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
    apply: bool,
    no_comment: bool,
    force: bool,
    ctx: &OutputContext,
    config: &AppConfig,
) -> Result<Option<types::TriageResult>> {
    // Phase 1a: Fetch issue
    let spinner = maybe_spinner(ctx, "Fetching issue...");
    let issue_details = triage::fetch(reference, repo_context).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Phase 1b: Check if already triaged (unless force is true)
    if !force {
        let triage_status = check_already_triaged(&issue_details);
        if triage_status.is_triaged() {
            if matches!(ctx.format, OutputFormat::Text) {
                println!("{}", style("Already triaged (skipping)").yellow());
            }
            return Ok(None);
        }
    }

    // Phase 1c: Analyze with AI
    let spinner = maybe_spinner(ctx, "Analyzing with AI...");
    let ai_response = triage::analyze(&issue_details).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Build result for rendering (before posting decision)
    let is_maintainer = issue_details
        .viewer_permission
        .as_ref()
        .is_some_and(|p| p == "Admin" || p == "Maintain" || p == "Write");

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
        is_maintainer,
    };

    // Render triage FIRST (before asking for confirmation)
    output::render(&result, ctx);

    // Handle dry-run - already rendered, just exit
    if dry_run {
        return Ok(Some(result));
    }

    // Determine if we should post a comment (independent of --apply)
    let should_post_comment = if no_comment {
        false
    } else if !ctx.is_interactive() && !yes {
        // For non-interactive without --yes, don't post (safe default)
        false
    } else if yes {
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

    // Phase 2: Post the comment (if not skipped)
    let comment_url = if should_post_comment {
        let spinner = maybe_spinner(ctx, "Posting comment...");
        let analyze_result = triage::AnalyzeResult {
            issue_details: issue_details.clone(),
            triage: ai_response.triage.clone(),
            ai_stats: ai_response.stats.clone(),
        };
        let url = triage::post(&analyze_result).await?;
        if let Some(s) = spinner {
            s.finish_and_clear();
        }
        Some(url)
    } else {
        if matches!(ctx.format, OutputFormat::Text) && !no_comment {
            println!("{}", style("Triage not posted.").yellow());
        }
        None
    };

    result.comment_url.clone_from(&comment_url);

    // Phase 3: Apply labels and milestone if requested (independent of comment posting)
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

    // Record to history only if comment was posted
    if let Some(url) = &comment_url {
        let contribution = aptu_core::history::Contribution {
            id: uuid::Uuid::new_v4(),
            repo: format!("{}/{}", issue_details.owner, issue_details.repo),
            issue: issue_details.number,
            action: "triage".to_string(),
            timestamp: chrono::Utc::now(),
            comment_url: url.clone(),
            status: aptu_core::history::ContributionStatus::Pending,
            ai_stats: Some(ai_response.stats),
        };
        aptu_core::history::add_contribution(contribution)?;
        debug!("Contribution recorded to history");
    }

    // Show success messages
    if matches!(ctx.format, OutputFormat::Text) {
        if let Some(url) = &comment_url {
            println!();
            println!("{}", style("Comment posted successfully!").green().bold());
            println!("  {}", style(url).cyan().underlined());
        }
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
                let result = auth::run_status().await?;
                output::render(&result, &ctx);
                Ok(())
            }
        },

        Commands::Repo(repo_cmd) => match repo_cmd {
            RepoCommand::List { curated, custom } => {
                let spinner = maybe_spinner(&ctx, "Fetching repositories...");
                let result = repo::run_list(curated, custom).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                result.render_with_context(&ctx);
                Ok(())
            }
            RepoCommand::Add { repo } => {
                let spinner = maybe_spinner(&ctx, "Adding repository...");
                let result = repo::run_add(&repo).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if matches!(ctx.format, OutputFormat::Text) {
                    println!("{}", style(result).green());
                }
                Ok(())
            }
            RepoCommand::Remove { repo } => {
                let spinner = maybe_spinner(&ctx, "Removing repository...");
                let result = repo::run_remove(&repo)?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if matches!(ctx.format, OutputFormat::Text) {
                    println!("{}", style(result).green());
                }
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
                result.render_with_context(&ctx);
                Ok(())
            }
            IssueCommand::Triage {
                references,
                repo,
                since,
                dry_run,
                yes,
                apply,
                no_comment,
                force,
            } => {
                // Determine repo context: --repo flag > default_repo config
                let repo_context = repo.as_deref().or(config.user.default_repo.as_deref());

                // Resolve issue numbers from references or --since flag
                let issue_refs = if !references.is_empty() {
                    references
                } else if let Some(since_date) = since {
                    // Fetch untriaged issues since the specified date
                    if repo_context.is_none() {
                        anyhow::bail!(
                            "--since requires --repo or default_repo config when no references provided"
                        );
                    }
                    let (owner, repo_name) = repo_context
                        .unwrap()
                        .split_once('/')
                        .context("Invalid repo format, expected 'owner/repo'")?;

                    // Parse the date to RFC3339 format
                    let rfc3339_date = crate::cli::parse_date_to_rfc3339(&since_date)?;

                    let spinner = maybe_spinner(&ctx, "Fetching issues needing triage...");
                    let client = aptu_core::github::auth::create_client()
                        .context("Failed to create GitHub client")?;
                    let untriaged_issues = aptu_core::github::issues::fetch_issues_needing_triage(
                        &client,
                        owner,
                        repo_name,
                        Some(&rfc3339_date),
                        force,
                    )
                    .await?;
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }

                    // Warn if pagination limit hit
                    if untriaged_issues.len() == 100 && matches!(ctx.format, OutputFormat::Text) {
                        println!(
                            "{}",
                            style("Warning: Fetched 100 issues (pagination limit). There may be more untriaged issues.")
                                .yellow()
                        );
                    }

                    untriaged_issues
                        .into_iter()
                        .map(|issue| format!("{}#{}", repo_context.unwrap(), issue.number))
                        .collect()
                } else {
                    Vec::new()
                };

                if issue_refs.is_empty() {
                    if matches!(ctx.format, OutputFormat::Text) {
                        println!("{}", style("No issues to triage.").yellow());
                    }
                    return Ok(());
                }

                // Check GitHub rate limit before triaging (only when we have issues)
                if aptu_core::github::auth::is_authenticated() {
                    let spinner = maybe_spinner(&ctx, "Checking GitHub rate limit...");
                    let gh_client = aptu_core::github::auth::create_client()
                        .context("Failed to create GitHub client")?;
                    let rate_limit = aptu_core::check_rate_limit(&gh_client).await?;
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }

                    if rate_limit.is_low() && matches!(ctx.format, OutputFormat::Text) {
                        println!(
                            "{}",
                            style(format!("Warning: {}", rate_limit.message())).yellow()
                        );
                    }
                }

                // Show OpenRouter credits in verbose mode
                if ctx.verbose && matches!(ctx.format, OutputFormat::Text) {
                    debug!("Verbose mode enabled, showing OpenRouter credits");
                    // Note: OpenRouter credits check would require additional API call
                    // For now, we just log that verbose mode is active
                }

                // Bulk triage loop with concurrent processing
                let mut bulk_result = types::BulkTriageResult {
                    succeeded: 0,
                    failed: 0,
                    skipped: 0,
                    outcomes: Vec::new(),
                };

                // Process issues concurrently with buffer_unordered(5) for rate limit awareness
                let total_issues = issue_refs.len();
                let ctx_clone = ctx.clone();
                let outcomes = stream::iter(issue_refs.iter().enumerate())
                    .map(|(idx, issue_ref)| {
                        let issue_ref = issue_ref.clone();
                        let ctx = ctx_clone.clone();
                        async move {
                            // Progress output for concurrent processing
                            if matches!(ctx.format, OutputFormat::Text) {
                                println!(
                                    "\n[{}/{}] Triaging {}",
                                    idx + 1,
                                    total_issues,
                                    style(&issue_ref).cyan()
                                );
                            }

                            // Triage single issue
                            let result = triage_single_issue(
                                &issue_ref,
                                repo_context,
                                dry_run,
                                yes,
                                apply,
                                no_comment,
                                force,
                                &ctx,
                                config,
                            )
                            .await;

                            (issue_ref, result)
                        }
                    })
                    .buffer_unordered(5)
                    .collect::<Vec<_>>()
                    .await;

                // Process results and update bulk_result
                for (issue_ref, result) in outcomes {
                    match result {
                        Ok(Some(triage_result)) => {
                            bulk_result.succeeded += 1;
                            bulk_result.outcomes.push((
                                issue_ref,
                                types::SingleTriageOutcome::Success(Box::new(triage_result)),
                            ));
                        }
                        Ok(None) => {
                            bulk_result.skipped += 1;
                            bulk_result.outcomes.push((
                                issue_ref,
                                types::SingleTriageOutcome::Skipped("Already triaged".to_string()),
                            ));
                        }
                        Err(e) => {
                            bulk_result.failed += 1;
                            bulk_result.outcomes.push((
                                issue_ref,
                                types::SingleTriageOutcome::Failed(e.to_string()),
                            ));
                            if matches!(ctx.format, OutputFormat::Text) {
                                println!("  {}", style(format!("Error: {e}")).red());
                            }
                        }
                    }
                }

                // Render bulk summary (only for multiple issues)
                if issue_refs.len() > 1 {
                    output::render(&bulk_result, &ctx);
                }

                Ok(())
            }
            IssueCommand::Create {
                repo,
                title,
                body,
                from,
                dry_run,
            } => {
                let spinner = maybe_spinner(&ctx, "Creating issue...");
                let result = create::run(repo, title, body, from, dry_run).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                output::render(&result, &ctx);
                Ok(())
            }
        },

        Commands::History => {
            let result = history::run()?;
            output::render(&result, &ctx);
            Ok(())
        }

        Commands::Pr(pr_cmd) => match pr_cmd {
            PrCommand::Review {
                reference,
                repo,
                comment,
                approve,
                request_changes,
                dry_run,
                yes,
            } => {
                let repo_context = repo.as_deref().or(config.user.default_repo.as_deref());

                // Determine review type from flags
                let review_type = if comment {
                    Some(aptu_core::ReviewEvent::Comment)
                } else if approve {
                    Some(aptu_core::ReviewEvent::Approve)
                } else if request_changes {
                    Some(aptu_core::ReviewEvent::RequestChanges)
                } else {
                    None
                };

                let spinner = maybe_spinner(&ctx, "Fetching PR and analyzing...");
                let result = pr::run(&reference, repo_context, review_type, dry_run, yes).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                output::render(&result, &ctx);
                Ok(())
            }
            PrCommand::Label {
                reference,
                repo,
                dry_run,
            } => {
                let repo_context = repo.as_deref().or(config.user.default_repo.as_deref());

                let spinner = maybe_spinner(&ctx, "Fetching PR and extracting labels...");
                let result = pr::run_label(&reference, repo_context, dry_run).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                output::render(&result, &ctx);
                Ok(())
            }
        },

        Commands::Completion(completion_cmd) => match completion_cmd {
            CompletionCommand::Generate { shell } => completion::run_generate(shell),
            CompletionCommand::Install { shell, dry_run } => {
                completion::run_install(shell, dry_run)
            }
        },
    }
}
