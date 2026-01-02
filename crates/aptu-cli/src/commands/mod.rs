// SPDX-License-Identifier: Apache-2.0

//! Command handlers for Aptu CLI.

pub mod auth;
pub mod completion;
pub mod create;
pub mod history;
pub mod issue;
pub mod models;
pub mod pr;
pub mod release;
pub mod repo;
pub mod triage;
pub mod types;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::debug;

use crate::cli::{
    AuthCommand, Commands, CompletionCommand, IssueCommand, IssueState, OutputContext,
    OutputFormat, PrCommand, RepoCommand,
};
use crate::commands::types::{BulkPrReviewResult, PrReviewResult, SinglePrReviewOutcome};
use crate::output;
use aptu_core::{AppConfig, State, check_already_triaged};

/// Creates a styled spinner (only if interactive).
fn maybe_spinner(ctx: &OutputContext, message: &str) -> Option<ProgressBar> {
    if ctx.is_interactive() {
        let s = ProgressBar::new_spinner();
        s.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg} ({elapsed:.cyan})")
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
    no_apply: bool,
    no_comment: bool,
    force: bool,
    ctx: &OutputContext,
    config: &AppConfig,
) -> Result<Option<types::TriageResult>> {
    // Phase 1a: Fetch issue
    let spinner = maybe_spinner(ctx, "Fetching issue...");
    let fetch_start = Instant::now();
    let issue_details = triage::fetch(reference, repo_context).await?;
    let fetch_elapsed = fetch_start.elapsed();
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Phase 1a.5: Display issue preview (title and labels) immediately after fetch
    crate::output::common::show_preview(ctx, &issue_details.title, &issue_details.labels);

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
    let ai_response = triage::analyze(&issue_details, &config.ai).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Verbose output: show fetch timing and AI analysis timing
    crate::output::common::show_timing(
        ctx,
        fetch_elapsed.as_millis(),
        &ai_response.stats.model,
        ai_response.stats.duration_ms,
        ai_response.stats.input_tokens,
        ai_response.stats.output_tokens,
    );

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
    output::render(&result, ctx)?;

    // Handle dry-run - already rendered, just exit
    if dry_run {
        return Ok(Some(result));
    }

    // Determine if we should post a comment (independent of --apply)
    let should_post_comment = if no_comment {
        false
    } else if !ctx.is_interactive() {
        // For non-interactive mode, don't post (safe default)
        false
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
    if !no_apply {
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
        if !no_apply && (!result.applied_labels.is_empty() || result.applied_milestone.is_some()) {
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

/// Review a single PR and return the result.
///
/// Returns Ok(Some(result)) if reviewed successfully, Ok(None) if skipped,
/// or Err if an error occurred.
#[allow(clippy::too_many_arguments)]
async fn review_single_pr(
    reference: &str,
    repo_context: Option<&str>,
    review_type: Option<aptu_core::ReviewEvent>,
    dry_run: bool,
    yes: bool,
    ctx: &OutputContext,
    config: &AppConfig,
) -> Result<Option<PrReviewResult>> {
    // Fetch PR details
    let pr_details = pr::fetch(reference, repo_context).await?;

    // Display styled PR preview
    crate::output::common::show_preview(ctx, &pr_details.title, &pr_details.labels);

    // Analyze with AI
    let spinner = maybe_spinner(ctx, "Analyzing with AI...");
    let (review, ai_stats) = pr::analyze(&pr_details, &config.ai).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Build result
    let analyze_result = pr::AnalyzeResult {
        pr_details: pr_details.clone(),
        review: review.clone(),
        ai_stats: ai_stats.clone(),
    };

    // Handle posting if review type specified
    if let Some(event) = review_type {
        pr::post(
            &analyze_result,
            reference,
            repo_context,
            event,
            dry_run,
            yes,
        )
        .await?;
    }

    // Render output
    let result = PrReviewResult {
        pr_title: pr_details.title,
        pr_number: pr_details.number,
        pr_url: pr_details.url,
        review,
        ai_stats,
        dry_run,
        labels: pr_details.labels,
    };
    output::render(&result, ctx)?;

    Ok(Some(result))
}

/// Dispatch to the appropriate command handler.
#[allow(clippy::too_many_lines)]
pub async fn run(
    command: Commands,
    ctx: OutputContext,
    config: &AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    match command {
        Commands::Auth(auth_cmd) => match auth_cmd {
            AuthCommand::Login => auth::run_login().await,
            AuthCommand::Logout => auth::run_logout(),
            AuthCommand::Status => {
                let result = auth::run_status().await?;
                output::render(&result, &ctx)?;
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
                result.render_with_context(&ctx)?;
                Ok(())
            }
            RepoCommand::Discover {
                language,
                min_stars,
                limit,
            } => {
                let spinner = maybe_spinner(&ctx, "Discovering repositories...");
                let result = repo::run_discover(language, min_stars, limit).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                result.render_with_context(&ctx)?;
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
                result.render_with_context(&ctx)?;
                Ok(())
            }
            IssueCommand::Triage {
                references,
                repo,
                since,
                state,
                dry_run,
                no_apply,
                no_comment,
                force,
            } => {
                // Determine repo context: --repo flag > inferred_repo > default_repo config
                let repo_context = repo
                    .as_deref()
                    .or(inferred_repo.as_deref())
                    .or(config.user.default_repo.as_deref());

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

                    // Convert IssueState to octocrab::params::State
                    let octocrab_state = match state {
                        IssueState::Open => State::Open,
                        IssueState::Closed => State::Closed,
                        IssueState::All => State::All,
                    };

                    let spinner = maybe_spinner(&ctx, "Fetching issues needing triage...");
                    let client = aptu_core::github::auth::create_client()
                        .context("Failed to create GitHub client")?;
                    let untriaged_issues = aptu_core::github::issues::fetch_issues_needing_triage(
                        &client,
                        owner,
                        repo_name,
                        Some(&rfc3339_date),
                        force,
                        octocrab_state,
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

                // Bulk triage using core processor
                let items: Vec<(String, ())> = issue_refs.iter().map(|r| (r.clone(), ())).collect();

                let ctx_for_processor = ctx.clone();
                let ctx_for_progress = ctx.clone();
                let repo_context_owned = repo_context.map(std::string::ToString::to_string);
                let config_clone = config.clone();

                let core_result = aptu_core::process_bulk(
                    items,
                    move |(issue_ref, ())| {
                        let ctx = ctx_for_processor.clone();
                        let repo_context = repo_context_owned.clone();
                        let config = config_clone.clone();
                        async move {
                            triage_single_issue(
                                &issue_ref,
                                repo_context.as_deref(),
                                dry_run,
                                no_apply,
                                no_comment,
                                force,
                                &ctx,
                                &config,
                            )
                            .await
                        }
                    },
                    move |current, total, action| {
                        crate::output::common::show_progress(
                            &ctx_for_progress,
                            current,
                            total,
                            action,
                        );
                    },
                )
                .await;

                // Convert core BulkResult to CLI BulkTriageResult
                let mut bulk_result = types::BulkTriageResult {
                    succeeded: core_result.succeeded,
                    failed: core_result.failed,
                    skipped: core_result.skipped,
                    outcomes: Vec::new(),
                };

                for (issue_ref, outcome) in core_result.outcomes {
                    let cli_outcome = match outcome {
                        aptu_core::BulkOutcome::Success(triage_result) => {
                            types::SingleTriageOutcome::Success(Box::new(triage_result))
                        }
                        aptu_core::BulkOutcome::Skipped(msg) => {
                            types::SingleTriageOutcome::Skipped(msg)
                        }
                        aptu_core::BulkOutcome::Failed(err) => {
                            if matches!(ctx.format, OutputFormat::Text) {
                                println!("  {}", style(format!("Error: {err}")).red());
                            }
                            types::SingleTriageOutcome::Failed(err)
                        }
                    };
                    bulk_result.outcomes.push((issue_ref, cli_outcome));
                }

                // Render bulk summary (only for multiple issues)
                if issue_refs.len() > 1 {
                    output::render(&bulk_result, &ctx)?;
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
                output::render(&result, &ctx)?;
                Ok(())
            }
        },

        Commands::History => {
            let result = history::run()?;
            output::render(&result, &ctx)?;
            Ok(())
        }

        Commands::Pr(pr_cmd) => match pr_cmd {
            PrCommand::Review {
                references,
                repo,
                comment,
                approve,
                request_changes,
                dry_run,
                no_apply: _,
                no_comment: _,
                force: _,
            } => {
                let repo_context = repo
                    .as_deref()
                    .or(inferred_repo.as_deref())
                    .or(config.user.default_repo.as_deref());

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

                if references.is_empty() {
                    if matches!(ctx.format, OutputFormat::Text) {
                        println!("{}", style("No PRs to review.").yellow());
                    }
                    return Ok(());
                }

                // Bulk PR review using core processor
                let items: Vec<(String, ())> = references.iter().map(|r| (r.clone(), ())).collect();

                let ctx_for_processor = ctx.clone();
                let ctx_for_progress = ctx.clone();
                let repo_context_owned = repo_context.map(std::string::ToString::to_string);
                let config_clone = config.clone();

                let core_result = aptu_core::process_bulk(
                    items,
                    move |(pr_ref, ())| {
                        let ctx = ctx_for_processor.clone();
                        let repo_context = repo_context_owned.clone();
                        let config = config_clone.clone();
                        async move {
                            review_single_pr(
                                &pr_ref,
                                repo_context.as_deref(),
                                review_type,
                                dry_run,
                                false,
                                &ctx,
                                &config,
                            )
                            .await
                        }
                    },
                    move |current, total, action| {
                        crate::output::common::show_progress(
                            &ctx_for_progress,
                            current,
                            total,
                            action,
                        );
                    },
                )
                .await;

                // Convert core BulkResult to CLI BulkPrReviewResult
                let mut bulk_result = BulkPrReviewResult {
                    succeeded: core_result.succeeded,
                    failed: core_result.failed,
                    skipped: core_result.skipped,
                    outcomes: Vec::new(),
                };

                for (pr_ref, outcome) in core_result.outcomes {
                    let cli_outcome = match outcome {
                        aptu_core::BulkOutcome::Success(review_result) => {
                            SinglePrReviewOutcome::Success(Box::new(review_result))
                        }
                        aptu_core::BulkOutcome::Skipped(msg) => SinglePrReviewOutcome::Skipped(msg),
                        aptu_core::BulkOutcome::Failed(err) => {
                            if matches!(ctx.format, OutputFormat::Text) {
                                println!("  {}", style(format!("Error: {err}")).red());
                            }
                            SinglePrReviewOutcome::Failed(err)
                        }
                    };
                    bulk_result.outcomes.push((pr_ref, cli_outcome));
                }

                // Render bulk summary (only for multiple PRs)
                if references.len() > 1 {
                    output::render(&bulk_result, &ctx)?;
                }

                Ok(())
            }
            PrCommand::Label {
                reference,
                repo,
                dry_run,
            } => {
                let repo_context = repo
                    .as_deref()
                    .or(inferred_repo.as_deref())
                    .or(config.user.default_repo.as_deref());

                let spinner = maybe_spinner(&ctx, "Fetching PR and extracting labels...");
                let result = pr::run_label(&reference, repo_context, dry_run, &config.ai).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                output::render(&result, &ctx)?;
                Ok(())
            }
        },

        Commands::Release {
            tag,
            repo,
            from,
            to,
            unreleased,
            update,
            dry_run,
        } => {
            let spinner = maybe_spinner(&ctx, "Generating release notes...");
            let result = release::run_generate(
                tag.as_deref(),
                repo.as_deref(),
                from.as_deref(),
                to.as_deref(),
                unreleased,
                update,
                dry_run,
                &ctx,
            )
            .await?;
            if let Some(s) = spinner {
                s.finish_and_clear();
            }
            output::render(&result, &ctx)?;
            Ok(())
        }

        Commands::Models(models_cmd) => match models_cmd {
            crate::cli::ModelsCommand::List {
                provider,
                sort,
                min_context,
            } => {
                let spinner = maybe_spinner(&ctx, "Fetching models...");
                if let Some(provider_name) = provider {
                    // Single provider
                    let result = models::run_list(&provider_name, sort, min_context).await?;
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                    output::render(&result, &ctx)?;
                } else {
                    // All providers
                    let result = models::run_list_all().await?;
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                    output::render(&result, &ctx)?;
                }
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
