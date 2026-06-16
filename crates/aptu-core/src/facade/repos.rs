// SPDX-License-Identifier: Apache-2.0

//! Repository management facade functions.

#[cfg(not(target_arch = "wasm32"))]
use chrono::Duration;
#[cfg(not(target_arch = "wasm32"))]
use secrecy::SecretString;
#[cfg(not(target_arch = "wasm32"))]
use tracing::instrument;

#[cfg(not(target_arch = "wasm32"))]
use crate::auth::TokenProvider;
#[cfg(not(target_arch = "wasm32"))]
use crate::cache::{FileCache, FileCacheImpl};
#[cfg(not(target_arch = "wasm32"))]
use crate::config::load_config;
use crate::error::AptuError;
#[cfg(not(target_arch = "wasm32"))]
use crate::github::auth::create_client_from_provider;
#[cfg(not(target_arch = "wasm32"))]
use crate::github::graphql::IssueNode;
#[cfg(not(target_arch = "wasm32"))]
use crate::github::graphql::fetch_issues as gh_fetch_issues;
use crate::repos::{self, CuratedRepo};

/// Fetches "good first issue" issues from curated repositories.
///
/// This function abstracts the credential resolution and API client creation,
/// allowing platforms to provide credentials via `TokenProvider` implementations.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `repo_filter` - Optional repository filter (case-insensitive substring match on full name or short name)
/// * `use_cache` - Whether to use cached results (if available and valid)
///
/// # Returns
///
/// A vector of `(repo_name, issues)` tuples.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - GitHub API call fails
/// - Response parsing fails
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(provider), fields(repo_filter = ?repo_filter, use_cache))]
pub async fn fetch_issues(
    provider: &dyn TokenProvider,
    repo_filter: Option<&str>,
    use_cache: bool,
) -> crate::Result<Vec<(String, Vec<IssueNode>)>> {
    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Get curated repos, optionally filtered
    let all_repos = repos::fetch().await?;
    let repos_to_query: Vec<_> = match repo_filter {
        Some(filter) => {
            let filter_lower = filter.to_lowercase();
            all_repos
                .iter()
                .filter(|r| {
                    r.full_name().to_lowercase().contains(&filter_lower)
                        || r.name.to_lowercase().contains(&filter_lower)
                })
                .cloned()
                .collect()
        }
        None => all_repos,
    };

    // Load config for cache TTL
    let config = load_config()?;
    let ttl = Duration::minutes(config.cache.issue_ttl_minutes);

    // Try to read from cache if enabled
    if use_cache {
        let cache: FileCacheImpl<Vec<IssueNode>> = FileCacheImpl::new("issues", ttl);
        let mut cached_results = Vec::new();
        let mut repos_to_fetch = Vec::new();

        for repo in &repos_to_query {
            let cache_key = format!("{}_{}", repo.owner, repo.name);
            if let Ok(Some(issues)) = cache.get(&cache_key).await {
                cached_results.push((repo.full_name(), issues));
            } else {
                repos_to_fetch.push(repo.clone());
            }
        }

        // If all repos are cached, return early
        if repos_to_fetch.is_empty() {
            return Ok(cached_results);
        }

        // Fetch missing repos from API - convert to tuples for GraphQL
        let repo_tuples: Vec<_> = repos_to_fetch
            .iter()
            .map(|r| (r.owner.as_str(), r.name.as_str()))
            .collect();
        let api_results =
            gh_fetch_issues(&client, &repo_tuples)
                .await
                .map_err(|e| AptuError::GitHub {
                    message: format!("Failed to fetch issues: {e}"),
                })?;

        // Write fetched results to cache
        for (repo_name, issues) in &api_results {
            if let Some(repo) = repos_to_fetch.iter().find(|r| r.full_name() == *repo_name) {
                let cache_key = format!("{}_{}", repo.owner, repo.name);
                let _ = cache.set(&cache_key, issues).await;
            }
        }

        // Combine cached and fetched results
        cached_results.extend(api_results);
        Ok(cached_results)
    } else {
        // Cache disabled, fetch directly from API - convert to tuples
        let repo_tuples: Vec<_> = repos_to_query
            .iter()
            .map(|r| (r.owner.as_str(), r.name.as_str()))
            .collect();
        gh_fetch_issues(&client, &repo_tuples)
            .await
            .map_err(|e| AptuError::GitHub {
                message: format!("Failed to fetch issues: {e}"),
            })
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn fetch_issues(
    _provider: &dyn crate::auth::TokenProvider,
    _repo_filter: Option<&str>,
    _use_cache: bool,
) -> crate::Result<Vec<(String, Vec<crate::github::graphql::IssueNode>)>> {
    crate::facade::wasm_unsupported!("fetch_issues");
}

/// Fetches curated repositories with platform-agnostic API.
///
/// This function provides a facade for fetching curated repositories,
/// allowing platforms (CLI, iOS, MCP) to use a consistent interface.
///
/// # Returns
///
/// A vector of `CuratedRepo` structs.
///
/// # Errors
///
/// Returns an error if configuration cannot be loaded.
#[cfg(not(target_arch = "wasm32"))]
pub async fn list_curated_repos() -> crate::Result<Vec<CuratedRepo>> {
    repos::fetch().await
}

#[cfg(target_arch = "wasm32")]
pub async fn list_curated_repos() -> crate::Result<Vec<CuratedRepo>> {
    Err(AptuError::GitHub {
        message: "list_curated_repos is not supported on wasm32-unknown-unknown".into(),
    })
}

/// Adds a custom repository.
///
/// Validates the repository via GitHub API and adds it to the custom repos file.
///
/// # Arguments
///
/// * `owner` - Repository owner
/// * `name` - Repository name
///
/// # Returns
///
/// The added `CuratedRepo`.
///
/// # Errors
///
/// Returns an error if:
/// - Repository cannot be found on GitHub
/// - Custom repos file cannot be read or written
#[cfg(not(target_arch = "wasm32"))]
#[instrument]
pub async fn add_custom_repo(owner: &str, name: &str) -> crate::Result<CuratedRepo> {
    // Validate and fetch metadata from GitHub
    let repo = repos::custom::validate_and_fetch_metadata(owner, name).await?;

    // Read existing custom repos
    let mut custom_repos = repos::custom::read_custom_repos()?;

    // Check if repo already exists
    if custom_repos
        .iter()
        .any(|r| r.full_name() == repo.full_name())
    {
        return Err(crate::error::AptuError::Config {
            message: format!(
                "Repository {} already exists in custom repos",
                repo.full_name()
            ),
        });
    }

    // Add new repo
    custom_repos.push(repo.clone());

    // Write back to file
    repos::custom::write_custom_repos(&custom_repos)?;

    Ok(repo)
}

#[cfg(target_arch = "wasm32")]
pub async fn add_custom_repo(owner: &str, name: &str) -> crate::Result<CuratedRepo> {
    let _ = (owner, name);
    Err(AptuError::GitHub {
        message: "add_custom_repo is not supported on wasm32-unknown-unknown".into(),
    })
}

/// Removes a custom repository.
///
/// # Arguments
///
/// * `owner` - Repository owner
/// * `name` - Repository name
///
/// # Returns
///
/// True if the repository was removed, false if it was not found.
///
/// # Errors
///
/// Returns an error if the custom repos file cannot be read or written.
#[cfg(not(target_arch = "wasm32"))]
#[instrument]
pub fn remove_custom_repo(owner: &str, name: &str) -> crate::Result<bool> {
    let full_name = format!("{owner}/{name}");

    // Read existing custom repos
    let mut custom_repos = repos::custom::read_custom_repos()?;

    // Find and remove the repo
    let initial_len = custom_repos.len();
    custom_repos.retain(|r| r.full_name() != full_name);

    if custom_repos.len() == initial_len {
        return Ok(false); // Not found
    }

    // Write back to file
    repos::custom::write_custom_repos(&custom_repos)?;

    Ok(true)
}

#[cfg(target_arch = "wasm32")]
pub fn remove_custom_repo(owner: &str, name: &str) -> crate::Result<bool> {
    let _ = (owner, name);
    Err(AptuError::GitHub {
        message: "remove_custom_repo is not supported on wasm32-unknown-unknown".into(),
    })
}

/// Lists repositories with optional filtering.
///
/// # Arguments
///
/// * `filter` - Repository filter (All, Curated, or Custom)
///
/// # Returns
///
/// A vector of `CuratedRepo` structs.
///
/// # Errors
///
/// Returns an error if repositories cannot be fetched.
#[cfg(not(target_arch = "wasm32"))]
#[instrument]
pub async fn list_repos(filter: repos::RepoFilter) -> crate::Result<Vec<CuratedRepo>> {
    repos::fetch_all(filter).await
}

#[cfg(target_arch = "wasm32")]
pub async fn list_repos(filter: repos::RepoFilter) -> crate::Result<Vec<CuratedRepo>> {
    let _ = filter;
    Err(AptuError::GitHub {
        message: "list_repos is not supported on wasm32-unknown-unknown".into(),
    })
}

/// Discovers repositories matching a filter via GitHub Search API.
///
/// Searches GitHub for welcoming repositories with good first issue or help wanted labels.
/// Results are scored client-side and cached with 24-hour TTL.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `filter` - Discovery filter (language, `min_stars`, `limit`)
///
/// # Returns
///
/// A vector of discovered repositories, sorted by relevance score.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - GitHub API call fails
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(provider), fields(language = ?filter.language, min_stars = filter.min_stars, limit = filter.limit))]
pub async fn discover_repos(
    provider: &dyn TokenProvider,
    filter: repos::discovery::DiscoveryFilter,
) -> crate::Result<Vec<repos::discovery::DiscoveredRepo>> {
    let token = provider.github_token().ok_or(AptuError::NotAuthenticated)?;
    let token = SecretString::from(token);
    repos::discovery::search_repositories(&token, &filter).await
}

#[cfg(target_arch = "wasm32")]
pub async fn discover_repos(
    _provider: &dyn crate::auth::TokenProvider,
    _filter: crate::repos::discovery::DiscoveryFilter,
) -> crate::Result<Vec<crate::repos::discovery::DiscoveredRepo>> {
    crate::facade::wasm_unsupported!("discover_repos");
}
