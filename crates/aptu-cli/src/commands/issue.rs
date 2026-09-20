// SPDX-License-Identifier: Apache-2.0

//! Issue command handlers.

use anyhow::{Context, Result};
use console::style;

use crate::cli::{IssueCommand, IssueState, OutputContext, OutputFormat};
use crate::commands::common::maybe_spinner;
use crate::commands::types;
use aptu_core::AppConfig;

/// Run the issue triage command.
#[allow(clippy::too_many_lines)]
#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::too_many_arguments)]
pub async fn run_triage(
    references: Vec<String>,
    repo: Option<String>,
    since: Option<String>,
    state: IssueState,
    dry_run: bool,
    no_apply: bool,
    no_comment: bool,
    force: bool,
    ctx: OutputContext,
    config: &AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    // Determine repo context: --repo flag > inferred_repo > default_repo config
    let repo_context = repo
        .as_deref()
        .or(inferred_repo.as_deref())
        .or(config.user.default_repo.as_deref());

    // Resolve issue numbers from references or --since flag
    let issue_refs = if references.is_empty() {
        super::resolve_triage_refs(since, state, repo_context, force, &ctx).await?
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
        let gh_client =
            aptu_core::github::auth::create_client().context("Failed to create GitHub client")?;
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
                super::triage_single_issue(
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
        let cli_outcome = super::report_outcome(outcome, &ctx);
        bulk_result.outcomes.push((issue_ref, cli_outcome));
    }

    // Render bulk summary (only for multiple issues)
    if issue_refs.len() > 1 {
        crate::output::render(&bulk_result, &ctx)?;
    }

    Ok(())
}

/// Dispatch an issue subcommand.
pub(crate) async fn run(
    issue_cmd: IssueCommand,
    ctx: OutputContext,
    config: &AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    match issue_cmd {
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
            run_triage(
                references,
                repo,
                since,
                state,
                dry_run,
                no_apply,
                no_comment,
                force,
                ctx,
                config,
                inferred_repo,
            )
            .await
        }
    }
}
