// SPDX-License-Identifier: Apache-2.0

//! Platform-agnostic facade functions for FFI and CLI integration.
//!
//! This module provides high-level functions that abstract away the complexity
//! of credential resolution, API client creation, and data transformation.
//! Each platform (CLI, iOS, MCP) implements `TokenProvider` and calls these
//! functions with their own credential source.

use chrono::Duration;
use tracing::instrument;

use crate::ai::OpenRouterClient;
use crate::ai::types::{IssueDetails, TriageResponse};
use crate::auth::TokenProvider;
use crate::cache::{self, CacheEntry};
use crate::config::load_config;
use crate::error::AptuError;
use crate::github::graphql::{IssueNode, fetch_issues as gh_fetch_issues};
use crate::repos;

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
#[instrument(skip(provider), fields(repo_filter = ?repo_filter, use_cache))]
pub async fn fetch_issues(
    provider: &dyn TokenProvider,
    repo_filter: Option<&str>,
    use_cache: bool,
) -> crate::Result<Vec<(String, Vec<IssueNode>)>> {
    // Get GitHub token from provider
    let github_token = provider.github_token().ok_or(AptuError::NotAuthenticated)?;

    // Create GitHub client with the provided token
    let client = octocrab::OctocrabBuilder::new()
        .personal_token(github_token)
        .build()
        .map_err(AptuError::GitHub)?;

    // Get curated repos, optionally filtered
    let all_repos = repos::list();
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
        None => all_repos.to_vec(),
    };

    // Load config for cache TTL
    let config = load_config()?;
    let ttl = Duration::minutes(config.cache.issue_ttl_minutes.try_into().unwrap_or(60));

    // Try to read from cache if enabled
    if use_cache {
        let mut cached_results = Vec::new();
        let mut repos_to_fetch = Vec::new();

        for repo in &repos_to_query {
            let cache_key = cache::cache_key_issues(repo.owner, repo.name);
            match cache::read_cache::<Vec<IssueNode>>(&cache_key) {
                Ok(Some(entry)) if entry.is_valid(ttl) => {
                    cached_results.push((repo.full_name(), entry.data));
                }
                _ => {
                    repos_to_fetch.push(repo.clone());
                }
            }
        }

        // If all repos are cached, return early
        if repos_to_fetch.is_empty() {
            return Ok(cached_results);
        }

        // Fetch missing repos from API
        let api_results = gh_fetch_issues(&client, &repos_to_fetch)
            .await
            .map_err(|e| AptuError::AI {
                message: format!("Failed to fetch issues: {e}"),
                status: None,
            })?;

        // Write fetched results to cache
        for (repo_name, issues) in &api_results {
            if let Some(repo) = repos_to_fetch.iter().find(|r| r.full_name() == *repo_name) {
                let cache_key = cache::cache_key_issues(repo.owner, repo.name);
                let entry = CacheEntry::new(issues.clone());
                let _ = cache::write_cache(&cache_key, &entry);
            }
        }

        // Combine cached and fetched results
        cached_results.extend(api_results);
        Ok(cached_results)
    } else {
        // Cache disabled, fetch directly from API
        gh_fetch_issues(&client, &repos_to_query)
            .await
            .map_err(|e| AptuError::AI {
                message: format!("Failed to fetch issues: {e}"),
                status: None,
            })
    }
}

/// Analyzes a GitHub issue and generates triage suggestions.
///
/// This function abstracts the credential resolution and API client creation,
/// allowing platforms to provide credentials via `TokenProvider` implementations.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub and `OpenRouter` credentials
/// * `issue` - Issue details to analyze
///
/// # Returns
///
/// Structured triage response with summary, labels, questions, and duplicates.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub or `OpenRouter` token is not available from the provider
/// - AI API call fails
/// - Response parsing fails
#[instrument(skip(provider, issue), fields(issue_number = issue.number, repo = %format!("{}/{}", issue.owner, issue.repo)))]
pub async fn analyze_issue(
    provider: &dyn TokenProvider,
    issue: &IssueDetails,
) -> crate::Result<TriageResponse> {
    // Get OpenRouter API key from provider
    let api_key = provider
        .openrouter_key()
        .ok_or(AptuError::NotAuthenticated)?;

    // Load configuration
    let config = load_config()?;

    // Create AI client with the provided API key
    let ai_client =
        OpenRouterClient::with_api_key(api_key, &config.ai).map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
        })?;

    // Analyze the issue
    ai_client
        .analyze_issue(issue)
        .await
        .map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    struct MockTokenProvider {
        github_token: Option<SecretString>,
        openrouter_key: Option<SecretString>,
    }

    impl TokenProvider for MockTokenProvider {
        fn github_token(&self) -> Option<SecretString> {
            self.github_token.clone()
        }

        fn openrouter_key(&self) -> Option<SecretString> {
            self.openrouter_key.clone()
        }
    }
}
