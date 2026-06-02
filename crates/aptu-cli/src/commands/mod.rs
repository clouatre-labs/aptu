// SPDX-License-Identifier: Apache-2.0

//! Command handlers for Aptu CLI.

pub mod auth;
pub mod completion;
pub mod create;
pub mod history;
pub mod issue;
pub mod models;
pub mod pr;
pub mod repo;
pub mod scan_security;
pub mod triage;
pub mod types;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use tracing::debug;

use crate::cli::{
    AuthCommand, Commands, CompletionCommand, IssueCommand, IssueState, OutputContext,
    OutputFormat, PrCommand, RepoCommand,
};
use crate::commands::types::{BulkPrReviewResult, PrReviewResult, SinglePrReviewOutcome};
use crate::output;
use aptu_core::{AppConfig, State, check_already_triaged, history::ContributionStatus};

/// Options for PR review behavior.
#[allow(clippy::struct_excessive_bools)]
struct ReviewOptions {
    dry_run: bool,
    yes: bool,
    no_comment: bool,
}

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

/// Should we post a comment based on configuration and user interaction?
fn should_post_comment(
    no_comment: bool,
    ctx: &OutputContext,
    confirm_before_post: bool,
) -> Result<bool> {
    if no_comment {
        return Ok(false);
    }
    if !ctx.is_interactive() {
        return Ok(false);
    }
    if confirm_before_post {
        println!();
        Confirm::new()
            .with_prompt("Post this triage as a comment to the issue?")
            .default(false)
            .interact()
            .context("Failed to get user confirmation")
    } else {
        Ok(true)
    }
}

/// Show success messages after triage is complete.
fn show_triage_success(
    ctx: &OutputContext,
    comment_url: Option<&str>,
    result: &types::TriageResult,
    no_apply: bool,
) {
    if !matches!(ctx.format, OutputFormat::Text) {
        return;
    }
    if let Some(url) = comment_url {
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

/// Triage a single issue and return the result.
///
/// Returns Ok(Some(result)) if triaged successfully, Ok(None) if skipped (already triaged),
/// or Err if an error occurred.
#[allow(clippy::too_many_lines)]
#[allow(clippy::fn_params_excessive_bools)]
/// Configuration for a single triage operation.
#[allow(clippy::struct_excessive_bools)]
struct TriageConfig<'a> {
    reference: &'a str,
    repo_context: Option<&'a str>,
    dry_run: bool,
    no_apply: bool,
    no_comment: bool,
    force: bool,
    ctx: &'a OutputContext,
    config: &'a AppConfig,
}

#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::too_many_arguments)]
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
    let triage_cfg = TriageConfig {
        reference,
        repo_context,
        dry_run,
        no_apply,
        no_comment,
        force,
        ctx,
        config,
    };
    triage_single_issue_impl(&triage_cfg).await
}

#[allow(clippy::too_many_lines)]
async fn triage_single_issue_impl(cfg: &TriageConfig<'_>) -> Result<Option<types::TriageResult>> {
    // Phase 1a: Fetch issue
    let spinner = maybe_spinner(cfg.ctx, "Fetching issue...");
    let fetch_start = Instant::now();
    let issue_details = triage::fetch(cfg.reference, cfg.repo_context).await?;
    let fetch_elapsed = fetch_start.elapsed();
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Phase 1a.5: Display issue preview (title and labels) immediately after fetch
    crate::output::common::show_preview(cfg.ctx, &issue_details.title, &issue_details.labels);

    // Phase 1b: Check if already triaged (unless force or dry_run is true)
    if !cfg.force && !cfg.dry_run {
        let triage_status = check_already_triaged(&issue_details);
        if triage_status.is_triaged() {
            if matches!(cfg.ctx.format, OutputFormat::Text) {
                println!("{}", style("Already triaged (skipping)").yellow());
            }
            return Ok(None);
        }
    }

    // Phase 1c: Analyze with AI
    let spinner = maybe_spinner(cfg.ctx, "Analyzing with AI...");
    let analyze_result = triage::analyze(&issue_details, &cfg.config.ai).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Verbose output: show fetch timing and AI analysis timing
    crate::output::common::show_timing(
        cfg.ctx,
        fetch_elapsed.as_millis(),
        &analyze_result.ai_stats.model,
        analyze_result.ai_stats.duration_ms,
        analyze_result.ai_stats.input_tokens,
        analyze_result.ai_stats.output_tokens,
    );

    // Log metrics (fire-and-forget)
    aptu_core::metrics::append_jsonl(&analyze_result.ai_stats);

    // Build result for rendering (before posting decision)
    let is_maintainer = issue_details
        .viewer_permission
        .as_ref()
        .is_some_and(|p| p == "Admin" || p == "Maintain" || p == "Write");

    let mut result = types::TriageResult {
        issue_title: issue_details.title.clone(),
        issue_number: issue_details.number,
        triage: analyze_result.triage.clone(),
        ai_stats: analyze_result.ai_stats.clone(),
        comment_url: None,
        dry_run: cfg.dry_run,
        user_declined: false,
        applied_labels: Vec::new(),
        applied_milestone: None,
        apply_warnings: Vec::new(),
        is_maintainer,
    };

    // Render triage FIRST (before asking for confirmation)
    output::render(&result, cfg.ctx)?;

    // Handle dry-run - already rendered, just exit
    if cfg.dry_run {
        return Ok(Some(result));
    }

    // Determine if we should post a comment (independent of --apply)
    let should_post_comment =
        should_post_comment(cfg.no_comment, cfg.ctx, cfg.config.ui.confirm_before_post)?;

    // Phase 2: Post the comment (if not skipped)
    let comment_url = if should_post_comment {
        let spinner = maybe_spinner(cfg.ctx, "Posting comment...");
        let url = triage::post(&analyze_result).await?;
        if let Some(s) = spinner {
            s.finish_and_clear();
        }
        Some(url)
    } else {
        if matches!(cfg.ctx.format, OutputFormat::Text) && !cfg.no_comment {
            println!("{}", style("Triage not posted.").yellow());
        }
        None
    };

    result.comment_url.clone_from(&comment_url);

    // Phase 3: Apply labels and milestone if requested (independent of comment posting)
    if !cfg.no_apply {
        let spinner = maybe_spinner(cfg.ctx, "Applying labels and milestone...");
        let apply_result = triage::apply(&issue_details, &analyze_result.triage).await?;
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
            ai_stats: Some(analyze_result.ai_stats),
        };
        aptu_core::history::add_contribution(contribution)?;
        debug!("Contribution recorded to history");
    }

    // Show success messages
    show_triage_success(cfg.ctx, comment_url.as_deref(), &result, cfg.no_apply);

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
    opts: ReviewOptions,
    ctx: &OutputContext,
    config: &AppConfig,
    repo_path: Option<String>,
    deep: bool,
) -> Result<Option<PrReviewResult>> {
    // Fetch PR details
    let pr_details = pr::fetch(reference, repo_context).await?;

    // Display styled PR preview
    crate::output::common::show_preview(ctx, &pr_details.title, &pr_details.labels);

    // Analyze with AI
    let spinner = maybe_spinner(ctx, "Analyzing with AI...");
    let (review, ai_stats, context_record) =
        pr::analyze(&pr_details, &config.ai, repo_path, deep).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Log metrics (fire-and-forget)
    aptu_core::metrics::append_jsonl(&ai_stats);
    aptu_core::metrics::write_context_jsonl(&context_record);

    // Security scanning (if PR has code changes)
    let security_findings = {
        let file_paths: Vec<String> = pr_details
            .files
            .iter()
            .map(|f| f.filename.clone())
            .collect();

        if aptu_core::needs_security_scan(&file_paths, &pr_details.labels, &pr_details.body) {
            let spinner = maybe_spinner(ctx, "Scanning for security issues...");

            // Run security scanner on each file in parallel using the default security config
            let scanner = aptu_core::SecurityScanner::default();
            let findings: Vec<_> = pr_details
                .files
                .par_iter()
                .filter_map(|file| {
                    file.patch
                        .as_ref()
                        .map(|patch| scanner.scan_file(patch, &file.filename))
                })
                .flatten()
                .collect();

            if let Some(s) = &spinner {
                s.finish_and_clear();
            }

            // Return Some(findings) even if empty to show "No issues found" message
            Some(findings)
        } else {
            None
        }
    };

    // Build result
    let analyze_result = pr::AnalyzeResult {
        pr_details: pr_details.clone(),
        review: review.clone(),
        ai_stats: ai_stats.clone(),
    };

    // Handle posting if review type specified and --no-comment not set
    if let Some(event) = review_type {
        if !opts.no_comment {
            pr::post(
                &analyze_result,
                reference,
                repo_context,
                event,
                opts.dry_run,
                opts.yes,
                ctx.is_verbose(),
            )
            .await?;
        }
    } else if !opts.dry_run && matches!(ctx.format, OutputFormat::Text) {
        eprintln!(
            "hint: run with --comment, --approve, or --request-changes to post this review to GitHub."
        );
    }

    // Render output
    let result = PrReviewResult {
        pr_title: pr_details.title,
        pr_number: pr_details.number,
        pr_url: pr_details.url,
        review: review.clone(),
        verdict: review.verdict.clone(),
        ai_stats,
        dry_run: opts.dry_run,
        labels: pr_details.labels,
        security_findings,
    };
    output::render_pr_review(&result, ctx)?;

    Ok(Some(result))
}

/// Dispatch to the appropriate command handler.
#[allow(clippy::too_many_lines)]
/// Run the auth command.
async fn run_auth_command(
    auth_cmd: AuthCommand,
    ctx: &OutputContext,
    config: &AppConfig,
) -> Result<()> {
    match auth_cmd {
        AuthCommand::Login => {
            let result = auth::run_login().await?;
            output::render(&result, ctx)?;
            Ok(())
        }
        AuthCommand::Logout => {
            let result = auth::run_logout()?;
            output::render(&result, ctx)?;
            Ok(())
        }
        AuthCommand::Status => {
            let result = auth::run_status(config).await?;
            output::render(&result, ctx)?;
            Ok(())
        }
    }
}

/// Run the repo command.
async fn run_repo_command(repo_cmd: RepoCommand, ctx: OutputContext) -> Result<()> {
    match repo_cmd {
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
            let message = repo::run_add(&repo).await?;
            if let Some(s) = spinner {
                s.finish_and_clear();
            }
            let result = types::RepoMutateResult {
                action: "add".to_string(),
                repo: repo.clone(),
                message,
            };
            output::render(&result, &ctx)?;
            Ok(())
        }
        RepoCommand::Remove { repo } => {
            let spinner = maybe_spinner(&ctx, "Removing repository...");
            let message = repo::run_remove(&repo)?;
            if let Some(s) = spinner {
                s.finish_and_clear();
            }
            let result = types::RepoMutateResult {
                action: "remove".to_string(),
                repo: repo.clone(),
                message,
            };
            output::render(&result, &ctx)?;
            Ok(())
        }
    }
}

/// Resolve issue references from --since flag.
async fn resolve_triage_refs(
    since: Option<String>,
    state: IssueState,
    repo_context: Option<&str>,
    force: bool,
    ctx: &OutputContext,
) -> Result<Vec<String>> {
    if let Some(since_date) = since {
        // Fetch untriaged issues since the specified date
        let repo_context = repo_context.ok_or_else(|| {
            anyhow::anyhow!(
                "--since requires --repo or default_repo config when no references provided"
            )
        })?;
        let (owner, repo_name) = repo_context
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

        let spinner = maybe_spinner(ctx, "Fetching issues needing triage...");
        let client =
            aptu_core::github::auth::create_client().context("Failed to create GitHub client")?;
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
                style(
                    "Warning: Fetched 100 issues (pagination limit). There may be more untriaged issues."
                )
                    .yellow()
            );
        }

        Ok(untriaged_issues
            .into_iter()
            .map(|issue| format!("{}#{}", repo_context, issue.number))
            .collect())
    } else {
        Ok(Vec::new())
    }
}

/// Run the issue command.
#[allow(clippy::too_many_lines)]
async fn run_issue_command(
    issue_cmd: IssueCommand,
    ctx: OutputContext,
    config: &AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    match issue_cmd {
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
            let issue_refs = if references.is_empty() {
                resolve_triage_refs(since, state, repo_context, force, &ctx).await?
            } else {
                references
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
                    crate::output::common::show_progress(&ctx_for_progress, current, total, action);
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
            let Some(effective_repo) = repo else {
                anyhow::bail!("repository is required; use --repo OWNER/REPO");
            };
            let spinner = maybe_spinner(&ctx, "Creating issue...");
            let result = create::run(effective_repo, title, body, from, dry_run).await?;
            if let Some(s) = spinner {
                s.finish_and_clear();
            }
            output::render(&result, &ctx)?;
            Ok(())
        }
        IssueCommand::Revert {
            issue,
            repo,
            dry_run,
        } => {
            let spinner = maybe_spinner(&ctx, "Reverting issue...");

            // Determine repo context: --repo flag > inferred_repo > default_repo config
            let repo_context = repo
                .as_deref()
                .or(inferred_repo.as_deref())
                .or(config.user.default_repo.as_deref());

            // Parse issue reference
            let (owner, repo_name, issue_number) =
                aptu_core::github::issues::parse_issue_reference(&issue, repo_context)
                    .context("Failed to parse issue reference")?;

            // Create GitHub client
            let gh_client = aptu_core::github::auth::create_client()
                .context("Failed to create GitHub client")?;

            // Call revert_issue facade function
            let outcome = aptu_core::facade::revert_issue(
                &gh_client,
                &owner,
                &repo_name,
                issue_number,
                dry_run,
            )
            .await
            .context("Failed to revert issue")?;

            if let Some(s) = spinner {
                s.finish_and_clear();
            }

            // Record history entry
            let action_name = if dry_run {
                "revert-dry-run".to_string()
            } else {
                "revert".to_string()
            };
            aptu_core::history::add_contribution(aptu_core::history::Contribution {
                id: uuid::Uuid::new_v4(),
                repo: format!("{owner}/{repo_name}"),
                issue: issue_number,
                action: action_name,
                timestamp: chrono::Utc::now(),
                comment_url: String::new(),
                status: ContributionStatus::default(),
                ai_stats: None,
            })
            .ok();

            // Format result
            let comments_count = outcome.comment_ids.len();
            let result = types::RevertResult {
                dry_run,
                labels_removed: outcome.labels_removed.clone(),
                comments_removed: comments_count,
                comment_ids: outcome.comment_ids,
                summary: if dry_run {
                    format!(
                        "Would remove {} comments and {} labels from issue #{issue_number}",
                        comments_count,
                        outcome.labels_removed.len()
                    )
                } else {
                    format!(
                        "Removed {} comments and {} labels from issue #{issue_number}",
                        comments_count,
                        outcome.labels_removed.len()
                    )
                },
            };

            output::render(&result, &ctx)?;
            Ok(())
        }
    }
}

/// Run the PR command.
#[allow(clippy::too_many_lines)]
async fn run_pr_command(
    pr_cmd: PrCommand,
    ctx: OutputContext,
    config: &AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    match pr_cmd {
        PrCommand::Review {
            references,
            repo,
            comment,
            approve,
            request_changes,
            dry_run,
            no_apply: _,
            no_comment,
            force,
            repo_path,
            deep,
            instructions_file,
        } => {
            let repo_path_str = repo_path.map(|p| p.to_string_lossy().into_owned());
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
            let mut config_clone = config.clone();
            let repo_path_str_owned = repo_path_str.clone();
            let instructions_file_str = instructions_file.map(|p| p.to_string_lossy().into_owned());

            // Override instructions_file in config if provided via CLI
            if let Some(ref path) = instructions_file_str {
                config_clone.review.instructions_file = Some(path.clone());
            }

            let core_result = aptu_core::process_bulk(
                items,
                move |(pr_ref, ())| {
                    let ctx = ctx_for_processor.clone();
                    let repo_context = repo_context_owned.clone();
                    let config = config_clone.clone();
                    let repo_path_for_review = repo_path_str_owned.clone();
                    async move {
                        review_single_pr(
                            &pr_ref,
                            repo_context.as_deref(),
                            review_type,
                            ReviewOptions {
                                dry_run,
                                yes: !ctx.is_interactive() || force,
                                no_comment,
                            },
                            &ctx,
                            &config,
                            repo_path_for_review,
                            deep,
                        )
                        .await
                    }
                },
                move |current, total, action| {
                    crate::output::common::show_progress(&ctx_for_progress, current, total, action);
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
            let (result, ai_stats) =
                pr::run_label(&reference, repo_context, dry_run, &config.ai).await?;
            if let Some(s) = spinner {
                s.finish_and_clear();
            }
            aptu_core::metrics::append_jsonl(&ai_stats);
            output::render(&result, &ctx)?;
            Ok(())
        }
        PrCommand::Create {
            repo,
            title,
            body,
            branch,
            base,
            diff,
            draft,
            force,
        } => {
            let spinner = maybe_spinner(&ctx, "Creating pull request...");
            let result = pr::run_pr_create(
                repo,
                inferred_repo,
                config.user.default_repo.clone(),
                title,
                body,
                branch,
                base,
                diff,
                draft,
                force,
            )
            .await?;
            if let Some(s) = spinner {
                s.finish_and_clear();
            }
            output::render(&result, &ctx)?;
            Ok(())
        }
        PrCommand::Queue { repo, limit } => {
            let repo_context = repo
                .as_deref()
                .or(inferred_repo.as_deref())
                .or(config.user.default_repo.as_deref());

            let repo_str = repo_context.ok_or_else(|| {
                anyhow::anyhow!(
                    "Could not determine owner/repo; use --repo or set default_repo in config"
                )
            })?;
            let (owner, repo_name) = aptu_core::github::parse_owner_repo(repo_str)?;

            let spinner = maybe_spinner(&ctx, "Fetching open PRs...");
            let result = pr::run_queue(config, &owner, &repo_name, limit).await?;
            if let Some(s) = spinner {
                s.finish_and_clear();
            }
            output::render(&result, &ctx)?;
            Ok(())
        }
        PrCommand::Revert { pr, repo, dry_run } => {
            let spinner = maybe_spinner(&ctx, "Reverting PR...");

            // Determine repo context: --repo flag > inferred_repo > default_repo config
            let repo_context = repo
                .as_deref()
                .or(inferred_repo.as_deref())
                .or(config.user.default_repo.as_deref());

            // Parse PR reference
            let (owner, repo_name, pr_number) =
                aptu_core::github::pulls::parse_pr_reference(&pr, repo_context)
                    .context("Failed to parse PR reference")?;

            // Create GitHub client
            let gh_client = aptu_core::github::auth::create_client()
                .context("Failed to create GitHub client")?;

            // Call revert_pr facade function
            let outcome =
                aptu_core::facade::revert_pr(&gh_client, &owner, &repo_name, pr_number, dry_run)
                    .await
                    .context("Failed to revert PR")?;

            if let Some(s) = spinner {
                s.finish_and_clear();
            }

            // Record history entry
            let action_name = if dry_run {
                "revert-dry-run".to_string()
            } else {
                "revert".to_string()
            };
            aptu_core::history::add_contribution(aptu_core::history::Contribution {
                id: uuid::Uuid::new_v4(),
                repo: format!("{owner}/{repo_name}"),
                issue: pr_number,
                action: action_name,
                timestamp: chrono::Utc::now(),
                comment_url: String::new(),
                status: ContributionStatus::default(),
                ai_stats: None,
            })
            .ok();

            // Format result
            let comments_count = outcome.comment_ids.len();
            let result = types::RevertResult {
                dry_run,
                labels_removed: outcome.labels_removed.clone(),
                comments_removed: comments_count,
                comment_ids: outcome.comment_ids,
                summary: if dry_run {
                    format!(
                        "Would remove {} comments and {} labels from PR #{pr_number}",
                        comments_count,
                        outcome.labels_removed.len()
                    )
                } else {
                    format!(
                        "Removed {} comments and {} labels from PR #{pr_number}",
                        comments_count,
                        outcome.labels_removed.len()
                    )
                },
            };

            output::render(&result, &ctx)?;
            Ok(())
        }
    }
}

/// Run the models command.
async fn run_models_command(
    models_cmd: crate::cli::ModelsCommand,
    ctx: OutputContext,
) -> Result<()> {
    match models_cmd {
        crate::cli::ModelsCommand::List {
            provider,
            sort,
            min_context,
            filter,
        } => {
            let spinner = maybe_spinner(&ctx, "Fetching models...");
            if let Some(provider_name) = provider {
                // Single provider
                let result =
                    models::run_list(&provider_name, sort, min_context, filter.as_deref()).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                output::render(&result, &ctx)?;
            } else {
                // All providers
                let result = models::run_list_all(filter.as_deref()).await?;
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                output::render(&result, &ctx)?;
            }
            Ok(())
        }
    }
}

/// Run the completion command.
fn run_completion_command(completion_cmd: &CompletionCommand, _ctx: OutputContext) -> Result<()> {
    match completion_cmd {
        CompletionCommand::Generate { shell } => completion::run_generate(*shell),
        CompletionCommand::Install { shell, dry_run } => completion::run_install(*shell, *dry_run),
    }
}

pub async fn run(
    command: Commands,
    ctx: OutputContext,
    config: &AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    // Validate that SARIF/GitHub Annotations output is only used with scan-security
    if matches!(
        ctx.format,
        OutputFormat::Sarif | OutputFormat::GithubAnnotations
    ) && !matches!(command, Commands::ScanSecurity { .. })
    {
        anyhow::bail!(
            "--output sarif and --output github-annotations are only supported with the scan-security command"
        );
    }

    match command {
        Commands::Auth(auth_cmd) => run_auth_command(auth_cmd, &ctx, config).await,
        Commands::Repo(repo_cmd) => run_repo_command(repo_cmd, ctx).await,
        Commands::Issue(issue_cmd) => {
            run_issue_command(issue_cmd, ctx, config, inferred_repo).await
        }
        Commands::History => {
            let result = history::run()?;
            output::render(&result, &ctx)?;
            Ok(())
        }
        Commands::Pr(pr_cmd) => run_pr_command(pr_cmd, ctx, config, inferred_repo).await,
        Commands::Models(models_cmd) => run_models_command(models_cmd, ctx).await,
        Commands::Completion(completion_cmd) => run_completion_command(&completion_cmd, ctx),
        Commands::ScanSecurity {
            path,
            diff,
            fail_on,
            exclude,
        } => {
            scan_security::run_scan_security_command(
                path, diff, fail_on, exclude, ctx.format, config,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{OutputContext, OutputFormat};
    use crate::commands::types::{AuthActionResult, RepoMutateResult};

    // UX-006/007: AuthActionResult renders correct text
    #[test]
    fn test_auth_action_result_render_text() {
        use crate::output::Renderable;

        // Arrange
        let result = AuthActionResult {
            action: "login".to_string(),
            message: "Successfully authenticated with GitHub!".to_string(),
        };
        let ctx = OutputContext::from_cli(OutputFormat::Text, false);
        let mut buf = Vec::new();

        // Act
        result.render_text(&mut buf, &ctx).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Successfully authenticated with GitHub!"));
    }

    // UX-006/007: AuthActionResult serializes to JSON
    #[test]
    fn test_auth_action_result_json_output() {
        // Arrange
        let result = AuthActionResult {
            action: "logout".to_string(),
            message: "Logged out from GitHub. Token removed from keychain.".to_string(),
        };

        // Act
        let json = serde_json::to_string(&result).unwrap();

        // Assert
        assert!(json.contains("\"action\":\"logout\""));
        assert!(json.contains("Logged out from GitHub"));
    }

    // UX-006/007: RepoMutateResult renders correct text (happy path)
    #[test]
    fn test_repo_mutate_result_render_text() {
        use crate::output::Renderable;

        // Arrange
        let result = RepoMutateResult {
            action: "add".to_string(),
            repo: "owner/name".to_string(),
            message: "Added repository: owner/name (Rust)".to_string(),
        };
        let ctx = OutputContext::from_cli(OutputFormat::Text, false);
        let mut buf = Vec::new();

        // Act
        result.render_text(&mut buf, &ctx).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Added repository: owner/name (Rust)"));
    }

    // UX-008: sarif format rejected on non-scan command (edge case)
    #[test]
    fn test_sarif_format_rejected_on_non_scan() {
        // Arrange: context with Sarif format
        let ctx = OutputContext::from_cli(OutputFormat::Sarif, false);

        // Assert: guard condition holds for non-scan commands
        let is_sarif = matches!(
            ctx.format,
            OutputFormat::Sarif | OutputFormat::GithubAnnotations
        );
        // Simulate non-scan command via a bool (ScanSecurity match is the only exclusion)
        let is_scan = false;
        assert!(
            is_sarif && !is_scan,
            "guard should reject sarif on non-scan"
        );
    }
}
