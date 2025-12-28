// SPDX-License-Identifier: Apache-2.0

//! Release notes generation command handler.

use anyhow::Result;

use crate::cli::OutputContext;
use crate::provider::CliTokenProvider;

/// Thin wrapper around `ReleaseNotesResponse` with `dry_run` flag.
#[derive(Debug, Clone)]
pub struct ReleaseNotesOutput {
    /// The release notes response from core
    pub response: aptu_core::ReleaseNotesResponse,
    /// Whether this is a dry run
    pub dry_run: bool,
}

/// Generate release notes for a repository.
pub async fn run_generate(
    repo: Option<&str>,
    from_tag: Option<&str>,
    to_tag: Option<&str>,
    dry_run: bool,
    _ctx: &OutputContext,
) -> Result<ReleaseNotesOutput> {
    // Require repo to be provided (can be enhanced later to infer from git)
    let repo_str = repo.ok_or_else(|| {
        anyhow::anyhow!("Repository must be provided in owner/repo format (--repo owner/repo)")
    })?;

    // Parse repo
    let (owner, repo_name) = repo_str
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("Repository must be in owner/repo format"))?;

    // Create token provider
    let token_provider = CliTokenProvider;

    // Generate release notes via facade
    let response =
        aptu_core::generate_release_notes(&token_provider, owner, repo_name, from_tag, to_tag)
            .await?;

    Ok(ReleaseNotesOutput { response, dry_run })
}
