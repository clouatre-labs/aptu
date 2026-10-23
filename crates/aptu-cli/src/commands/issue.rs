// SPDX-License-Identifier: Apache-2.0

//! Issue command handlers.

use anyhow::{Context, Result};
use console::style;

use crate::cli::{IssueCommand, OutputContext, OutputFormat};
use crate::commands::common::maybe_spinner;
use aptu_core::AppConfig;

/// Run the issue triage command.
#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::too_many_arguments)]
pub async fn run_triage(
    references: Vec<String>,
    repo: Option<String>,
    dry_run: bool,
    no_apply: bool,
    no_comment: bool,
    force: bool,
    ctx: OutputContext,
    config: &AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    if references.is_empty() {
        anyhow::bail!("No issue references provided. Usage: aptu issue triage <REFERENCE>");
    }

    // Determine repo context: --repo flag > inferred_repo > default_repo config
    let repo_context = repo
        .as_deref()
        .or(inferred_repo.as_deref())
        .or(config.user.default_repo.as_deref());

    // Check GitHub rate limit before triaging
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

    // Triage each reference sequentially, reporting failures without aborting the run
    let mut failures = 0;
    for issue_ref in &references {
        if let Err(err) = super::triage_single_issue(
            issue_ref,
            repo_context,
            dry_run,
            no_apply,
            no_comment,
            force,
            &ctx,
            config,
        )
        .await
        {
            failures += 1;
            if matches!(ctx.format, OutputFormat::Text) {
                println!("{}", style(format!("Error: {err:#}")).red());
            }
        }
    }

    if failures == references.len() && !matches!(ctx.format, OutputFormat::Text) {
        anyhow::bail!("All {failures} triage operations failed");
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
            dry_run,
            no_apply,
            no_comment,
            force,
        } => {
            run_triage(
                references,
                repo,
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
