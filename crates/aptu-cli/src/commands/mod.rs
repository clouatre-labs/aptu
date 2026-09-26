// SPDX-License-Identifier: Apache-2.0

//! Command handlers for Aptu CLI.

pub mod auth;
pub mod common;
pub mod completion;
pub mod issue;
pub mod issue_lint;
pub mod pr;
pub mod scan_security;
pub mod triage;
pub mod types;
pub mod workflow;

use std::time::Instant;

use anyhow::{Context, Result};
use console::style;
use dialoguer::Confirm;
use rayon::prelude::*;
use tracing::debug;

use crate::cli::{
    AuthCommand, Commands, CompletionCommand, IssueCommand, IssueState, OutputContext,
    OutputFormat, PrCommand,
};
use crate::commands::common::maybe_spinner;
use crate::commands::types::{OutcomeInfo, PrReviewResult};
use crate::output;
use aptu_core::{AppConfig, State, check_already_triaged};

/// Options for PR review behavior.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct ReviewOptions {
    pub dry_run: bool,
    pub yes: bool,
    pub no_comment: bool,
    pub no_dedup_summary: bool,
}

/// Convert a core bulk outcome into a CLI single outcome, reporting
/// per-item errors only in Text format (silent in JSON).
pub(crate) fn report_outcome<T: OutcomeInfo>(
    outcome: aptu_core::BulkOutcome<T::Inner>,
    ctx: &OutputContext,
) -> T {
    match outcome {
        aptu_core::BulkOutcome::Success(result) => T::from_success(result),
        aptu_core::BulkOutcome::Skipped(msg) => T::from_skipped(msg),
        aptu_core::BulkOutcome::Failed(err) => {
            if matches!(ctx.format, OutputFormat::Text) {
                println!("  {}", style(format!("Error: {err}")).red());
            }
            T::from_failed(err)
        }
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
pub(crate) async fn triage_single_issue(
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
        post_triage_comment(&analyze_result, &issue_details, cfg.ctx).await?
    } else {
        if matches!(cfg.ctx.format, OutputFormat::Text) && !cfg.no_comment {
            println!("{}", style("Triage not posted.").yellow());
        }
        None
    };

    result.comment_url.clone_from(&comment_url);

    // Phase 3: Apply labels and milestone if requested (independent of comment posting)
    if !cfg.no_apply {
        apply_triage_labels(&issue_details, &analyze_result.triage, cfg.ctx, &mut result).await?;
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

/// Post a triage comment, returning the comment URL if posted.
///
/// Returns `None` with a warning when the viewer lacks write access.
async fn post_triage_comment(
    analyze_result: &triage::AnalyzeResult,
    issue_details: &aptu_core::IssueDetails,
    ctx: &OutputContext,
) -> Result<Option<String>> {
    let spinner = maybe_spinner(ctx, "Posting comment...");
    let outcome = triage::post(analyze_result).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }
    if outcome.is_skipped() {
        if matches!(ctx.format, OutputFormat::Text) {
            println!(
                "{}",
                style(format!(
                    "No write access to {}/{} - triage not posted",
                    issue_details.owner, issue_details.repo
                ))
                .yellow()
            );
        }
        return Ok(None);
    }
    Ok(outcome.applied().cloned())
}

/// Apply AI-suggested labels and milestone, updating the triage result.
///
/// Warns instead of applying when the viewer lacks write access.
async fn apply_triage_labels(
    issue_details: &aptu_core::IssueDetails,
    triage: &aptu_core::TriageResponse,
    ctx: &OutputContext,
    result: &mut types::TriageResult,
) -> Result<()> {
    let spinner = maybe_spinner(ctx, "Applying labels and milestone...");
    let apply_outcome = triage::apply(issue_details, triage).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    if apply_outcome.is_skipped() {
        if matches!(ctx.format, OutputFormat::Text) {
            println!(
                "{}",
                style(format!(
                    "No write access to {}/{} - labels not applied",
                    issue_details.owner, issue_details.repo
                ))
                .yellow()
            );
        }
    } else if let Some(apply_result) = apply_outcome.applied() {
        result
            .applied_labels
            .clone_from(&apply_result.applied_labels);
        result
            .applied_milestone
            .clone_from(&apply_result.applied_milestone);
        result.apply_warnings.clone_from(&apply_result.warnings);
    }
    Ok(())
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
) -> Result<Option<PrReviewResult>> {
    // Fetch PR details
    let pr_details = pr::fetch(reference, repo_context).await?;

    // Display styled PR preview
    crate::output::common::show_preview(ctx, &pr_details.title, &pr_details.labels);

    // Analyze with AI
    let spinner = maybe_spinner(ctx, "Analyzing with AI...");
    let (review, ai_stats, context_record) =
        pr::analyze(&pr_details, &config.ai, repo_path).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    // Warn about budget drops before logging metrics
    if !context_record.budget_drops.is_empty() {
        eprintln!(
            "warning: review context truncated -- {} section(s) dropped to fit prompt budget: {}.\n  Raise limits under [review] in ~/.config/aptu/config.toml",
            context_record.budget_drops.len(),
            context_record.budget_drops.join(", ")
        );
    }

    // Warn about per-file patches truncated or skipped at prompt assembly
    if !context_record.truncated_patch_files.is_empty() {
        eprintln!(
            "warning: patch(es) truncated or skipped to fit prompt budget: {}.\n  Raise limits under [review] in ~/.config/aptu/config.toml",
            context_record.truncated_patch_files.join(", ")
        );
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
                opts.no_dedup_summary,
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
        files_total: context_record.files_total,
        files_with_patch: context_record.files_with_patch,
    };
    output::render_pr_review(&result, ctx)?;

    Ok(Some(result))
}

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

/// Resolve issue references from --since flag.
pub(crate) async fn resolve_triage_refs(
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
async fn run_issue_command(
    issue_cmd: IssueCommand,
    ctx: OutputContext,
    config: &AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    issue::run(issue_cmd, ctx, config, inferred_repo).await
}

/// Run the PR command.
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
            no_apply,
            no_comment,
            force,
            repo_path,
            instructions_file,
            no_dedup_summary,
        } => {
            pr::run_review(
                references,
                repo,
                comment,
                approve,
                request_changes,
                dry_run,
                no_apply,
                no_comment,
                force,
                repo_path,
                instructions_file,
                ctx,
                config,
                inferred_repo,
                no_dedup_summary,
            )
            .await
        }
        PrCommand::Label {
            reference,
            repo,
            dry_run,
        } => pr::run_label_command(reference, repo, dry_run, ctx, config, inferred_repo).await,
        PrCommand::Queue { repo, limit } => {
            pr::run_queue_command(repo, limit, ctx, config, inferred_repo).await
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
    match command {
        Commands::Auth(auth_cmd) => run_auth_command(auth_cmd, &ctx, config).await,
        Commands::Issue(issue_cmd) => {
            run_issue_command(issue_cmd, ctx, config, inferred_repo).await
        }
        Commands::Pr(pr_cmd) => run_pr_command(pr_cmd, ctx, config, inferred_repo).await,
        Commands::Completion(completion_cmd) => run_completion_command(&completion_cmd, ctx),
        Commands::ScanSecurity {
            path,
            diff,
            fail_on,
            exclude,
            sarif_output,
        } => {
            scan_security::run_scan_security_command(
                path,
                diff,
                fail_on,
                exclude,
                ctx.format,
                sarif_output,
                config,
            )
            .await
        }
        Commands::LintIssue {
            file,
            issue_type,
            config: lint_config,
        } => {
            issue_lint::run_lint_issue_command(file, issue_type, lint_config, ctx.format, config)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{OutputContext, OutputFormat};
    use crate::commands::types::AuthActionResult;

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
}
