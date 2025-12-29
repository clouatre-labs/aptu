// SPDX-License-Identifier: Apache-2.0

//! Repository management commands.

use anyhow::Result;
use aptu_core::{
    DiscoveryFilter, RepoFilter, add_custom_repo, discover_repos, list_repos, remove_custom_repo,
};

use super::types::{DiscoverResult, ReposResult};
use crate::provider::CliTokenProvider;

/// List repositories available for contribution.
pub async fn run_list(curated: bool, custom: bool) -> Result<ReposResult> {
    let filter = match (curated, custom) {
        (true, false) => RepoFilter::Curated,
        (false, true) => RepoFilter::Custom,
        _ => RepoFilter::All,
    };

    let repos = list_repos(filter).await?;
    Ok(ReposResult { repos })
}

/// Discover welcoming repositories on GitHub.
pub async fn run_discover(
    language: Option<String>,
    min_stars: u32,
    limit: u32,
) -> Result<DiscoverResult> {
    let filter = DiscoveryFilter {
        language,
        min_stars,
        limit,
    };

    let discovered = discover_repos(&CliTokenProvider, filter).await?;
    Ok(DiscoverResult { repos: discovered })
}

/// Add a custom repository.
pub async fn run_add(repo: &str) -> Result<String> {
    let (owner, name) = repo
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("Repository must be in owner/name format, got: {repo}"))?;

    let added = add_custom_repo(owner, name).await?;
    Ok(format!(
        "Added repository: {} ({})",
        added.full_name(),
        added.language
    ))
}

/// Remove a custom repository.
pub fn run_remove(repo: &str) -> Result<String> {
    let (owner, name) = repo
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("Repository must be in owner/name format, got: {repo}"))?;

    let removed = remove_custom_repo(owner, name)?;
    if removed {
        Ok(format!("Removed repository: {owner}/{name}"))
    } else {
        Ok(format!(
            "Repository {owner}/{name} not found in custom repos"
        ))
    }
}
