// SPDX-License-Identifier: Apache-2.0

//! List open issues command.
//!
//! Fetches "good first issue" issues from curated repositories using
//! the facade layer with `CliTokenProvider` for credential resolution.

use anyhow::Result;
use aptu_core::error::AptuError;
use tracing::{debug, info, instrument};

use super::types::IssuesResult;
use crate::provider::CliTokenProvider;

/// List open issues suitable for contribution.
///
/// Fetches issues with "good first issue" label from all curated repositories
/// (or a specific one if `--repo` is provided).
#[instrument(skip_all, fields(repo_filter = ?repo))]
pub async fn run(repo: Option<String>) -> Result<IssuesResult> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Fetch issues via facade with optional repo filter
    let results = aptu_core::fetch_issues(&provider, repo.as_deref())
        .await
        .map_err(|e| match e {
            AptuError::NotAuthenticated => AptuError::NotAuthenticated,
            other => other,
        })?;

    // Count total issues
    let total_count: usize = results.iter().map(|(_, issues)| issues.len()).sum();
    let no_repos_matched = results.is_empty();

    info!(
        total_issues = total_count,
        repos = results.len(),
        "Found issues"
    );
    debug!("Issues listing complete");

    Ok(IssuesResult {
        issues_by_repo: results,
        total_count,
        repo_filter: repo,
        no_repos_matched,
    })
}
