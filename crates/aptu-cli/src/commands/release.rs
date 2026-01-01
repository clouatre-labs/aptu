// SPDX-License-Identifier: Apache-2.0

//! Release notes generation command handler.

use anyhow::Result;

use crate::cli::OutputContext;
use crate::provider::CliTokenProvider;

/// Thin wrapper around `ReleaseNotesResponse` with `dry_run` flag.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ReleaseNotesOutput {
    /// The release notes response from core
    pub response: aptu_core::ReleaseNotesResponse,
    /// Whether this is a dry run
    pub dry_run: bool,
    /// URL of the posted release (if --update was used)
    pub release_url: Option<String>,
}

/// Generate release notes for a repository.
#[allow(clippy::too_many_arguments)]
pub async fn run_generate(
    tag: Option<&str>,
    repo: Option<&str>,
    from_tag: Option<&str>,
    to_tag: Option<&str>,
    unreleased: bool,
    update: bool,
    dry_run: bool,
    _ctx: &OutputContext,
) -> Result<ReleaseNotesOutput> {
    // Validate --update requires a tag
    if update && tag.is_none() && !unreleased {
        return Err(anyhow::anyhow!(
            "--update requires either a positional TAG or --unreleased flag"
        ));
    }

    // Infer repo from git if not provided
    let repo_str = if let Some(r) = repo {
        r.to_string()
    } else {
        aptu_core::infer_repo_from_git().map_err(|e| anyhow::anyhow!("{e}"))?
    };

    // Parse repo
    let (owner, repo_name) = repo_str
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("Repository must be in owner/repo format"))?;

    // Determine from and to tags based on arguments
    let (from_ref, to_ref) = if unreleased {
        // --unreleased: from latest tag to HEAD
        (None, Some("HEAD"))
    } else if let Some(t) = tag {
        // Positional tag: from latest to this tag
        (None, Some(t))
    } else {
        // Use explicit --from and --to
        (from_tag, to_tag)
    };

    // Create token provider
    let token_provider = CliTokenProvider;

    // Generate release notes via facade
    let response =
        aptu_core::generate_release_notes(&token_provider, owner, repo_name, from_ref, to_ref)
            .await?;

    // Post to GitHub if --update is set
    let release_url = if update && !dry_run {
        let release_tag = tag.unwrap_or("HEAD");
        let body = aptu_core::render_release_notes_markdown(&response);
        let url =
            aptu_core::post_release_notes(&token_provider, owner, repo_name, release_tag, &body)
                .await?;
        Some(url)
    } else {
        None
    };

    Ok(ReleaseNotesOutput {
        response,
        dry_run,
        release_url,
    })
}
