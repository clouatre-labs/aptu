// SPDX-License-Identifier: Apache-2.0

//! Platform-agnostic facade functions for FFI and CLI integration.
//!
//! This module provides high-level functions that abstract away the complexity
//! of credential resolution, API client creation, and data transformation.
//! Each platform (CLI, iOS, MCP) implements `TokenProvider` and calls these
//! functions with their own credential source.

use chrono::Duration;
use tracing::instrument;

use crate::ai::provider::MAX_LABELS;
use crate::ai::types::{PrDetails, ReviewEvent};
use crate::ai::{AiClient, AiProvider, AiResponse, types::IssueDetails};
use crate::auth::TokenProvider;
use crate::cache::{self, CacheEntry};
use crate::config::load_config;
use crate::error::AptuError;
use crate::github::auth::create_client_with_token;
use crate::github::graphql::{
    IssueNode, fetch_issue_with_repo_context, fetch_issues as gh_fetch_issues,
};
use crate::github::issues::filter_labels_by_relevance;
use crate::github::pulls::{fetch_pr_details, post_pr_review as gh_post_pr_review};
use crate::repos::{self, CuratedRepo};
use secrecy::SecretString;

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
    let token = SecretString::from(github_token);
    let client = create_client_with_token(&token).map_err(|e| AptuError::AI {
        message: format!("Failed to create GitHub client: {e}"),
        status: None,
    })?;

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
    let ttl = Duration::minutes(config.cache.issue_ttl_minutes.try_into().unwrap_or(60));

    // Try to read from cache if enabled
    if use_cache {
        let mut cached_results = Vec::new();
        let mut repos_to_fetch = Vec::new();

        for repo in &repos_to_query {
            let cache_key = cache::cache_key_issues(&repo.owner, &repo.name);
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

        // Fetch missing repos from API - convert to tuples for GraphQL
        let repo_tuples: Vec<_> = repos_to_fetch
            .iter()
            .map(|r| (r.owner.as_str(), r.name.as_str()))
            .collect();
        let api_results =
            gh_fetch_issues(&client, &repo_tuples)
                .await
                .map_err(|e| AptuError::AI {
                    message: format!("Failed to fetch issues: {e}"),
                    status: None,
                })?;

        // Write fetched results to cache
        for (repo_name, issues) in &api_results {
            if let Some(repo) = repos_to_fetch.iter().find(|r| r.full_name() == *repo_name) {
                let cache_key = cache::cache_key_issues(&repo.owner, &repo.name);
                let entry = CacheEntry::new(issues.clone());
                let _ = cache::write_cache(&cache_key, &entry);
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
            .map_err(|e| AptuError::AI {
                message: format!("Failed to fetch issues: {e}"),
                status: None,
            })
    }
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
pub async fn list_curated_repos() -> crate::Result<Vec<CuratedRepo>> {
    repos::fetch().await
}

/// Analyzes a GitHub issue and generates triage suggestions.
///
/// This function abstracts the credential resolution and API client creation,
/// allowing platforms to provide credentials via `TokenProvider` implementations.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub and AI provider credentials
/// * `issue` - Issue details to analyze
///
/// # Returns
///
/// AI response with triage data and usage statistics.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub or AI provider token is not available from the provider
/// - AI API call fails
/// - Response parsing fails
#[instrument(skip(provider, issue), fields(issue_number = issue.number, repo = %format!("{}/{}", issue.owner, issue.repo)))]
pub async fn analyze_issue(
    provider: &dyn TokenProvider,
    issue: &IssueDetails,
) -> crate::Result<AiResponse> {
    // Load configuration
    let config = load_config()?;

    // Clone issue into mutable local variable for potential label enrichment
    let mut issue_mut = issue.clone();

    // Fetch repository labels via GraphQL if available_labels is empty and owner/repo are non-empty
    if issue_mut.available_labels.is_empty()
        && !issue_mut.owner.is_empty()
        && !issue_mut.repo.is_empty()
    {
        // Get GitHub token from provider
        if let Some(github_token) = provider.github_token() {
            let token = SecretString::from(github_token);
            if let Ok(client) = create_client_with_token(&token) {
                // Attempt to fetch issue with repo context to get repository labels
                if let Ok((_, repo_data)) = fetch_issue_with_repo_context(
                    &client,
                    &issue_mut.owner,
                    &issue_mut.repo,
                    issue_mut.number,
                )
                .await
                {
                    // Extract available labels from repository data (not issue labels)
                    issue_mut.available_labels =
                        repo_data.labels.nodes.into_iter().map(Into::into).collect();
                }
            }
        }
    }

    // Apply label filtering before AI analysis
    if !issue_mut.available_labels.is_empty() {
        issue_mut.available_labels =
            filter_labels_by_relevance(&issue_mut.available_labels, MAX_LABELS);
    }

    // Get API key from provider using the configured provider name
    let api_key = provider
        .ai_api_key(&config.ai.provider)
        .ok_or(AptuError::NotAuthenticated)?;

    // Create generic AI client with provided API key
    let ai_client =
        AiClient::with_api_key(&config.ai.provider, api_key, &config.ai).map_err(|e| {
            AptuError::AI {
                message: e.to_string(),
                status: None,
            }
        })?;

    ai_client
        .analyze_issue(&issue_mut)
        .await
        .map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
        })
}

/// Reviews a pull request and generates AI feedback.
///
/// This function abstracts the credential resolution and API client creation,
/// allowing platforms to provide credentials via `TokenProvider` implementations.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub and AI provider credentials
/// * `reference` - PR reference (URL, owner/repo#number, or number)
/// * `repo_context` - Optional repository context for bare numbers
///
/// # Returns
///
/// Tuple of (`PrDetails`, `PrReviewResponse`) with PR info and AI review.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub or AI provider token is not available from the provider
/// - PR cannot be fetched
/// - AI API call fails
#[instrument(skip(provider), fields(reference = %reference))]
pub async fn review_pr(
    provider: &dyn TokenProvider,
    reference: &str,
    repo_context: Option<&str>,
) -> crate::Result<(
    PrDetails,
    crate::ai::types::PrReviewResponse,
    crate::history::AiStats,
)> {
    use crate::github::pulls::parse_pr_reference;

    // Get GitHub token from provider
    let github_token = provider.github_token().ok_or(AptuError::NotAuthenticated)?;

    // Parse PR reference
    let (owner, repo, number) =
        parse_pr_reference(reference, repo_context).map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
        })?;

    // Create GitHub client with the provided token
    let token = SecretString::from(github_token);
    let client = create_client_with_token(&token).map_err(|e| AptuError::AI {
        message: format!("Failed to create GitHub client: {e}"),
        status: None,
    })?;

    // Fetch PR details
    let pr_details = fetch_pr_details(&client, &owner, &repo, number)
        .await
        .map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
        })?;

    // Load configuration
    let config = load_config()?;

    // Get API key from provider using the configured provider name
    let api_key = provider
        .ai_api_key(&config.ai.provider)
        .ok_or(AptuError::NotAuthenticated)?;

    // Create generic AI client with provided API key
    let ai_client =
        AiClient::with_api_key(&config.ai.provider, api_key, &config.ai).map_err(|e| {
            AptuError::AI {
                message: e.to_string(),
                status: None,
            }
        })?;

    // Review PR with AI (timing and stats are captured in provider)
    let (review, ai_stats) = ai_client
        .review_pr(&pr_details)
        .await
        .map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
        })?;

    Ok((pr_details, review, ai_stats))
}

/// Posts a PR review to GitHub.
///
/// This function abstracts the credential resolution and API client creation,
/// allowing platforms to provide credentials via `TokenProvider` implementations.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `reference` - PR reference (URL, owner/repo#number, or number)
/// * `repo_context` - Optional repository context for bare numbers
/// * `body` - Review comment text
/// * `event` - Review event type (Comment, Approve, or `RequestChanges`)
///
/// # Returns
///
/// Review ID on success.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - PR cannot be parsed or found
/// - User lacks write access to the repository
/// - API call fails
#[instrument(skip(provider), fields(reference = %reference, event = %event))]
pub async fn post_pr_review(
    provider: &dyn TokenProvider,
    reference: &str,
    repo_context: Option<&str>,
    body: &str,
    event: ReviewEvent,
) -> crate::Result<u64> {
    use crate::github::pulls::parse_pr_reference;

    // Get GitHub token from provider
    let github_token = provider.github_token().ok_or(AptuError::NotAuthenticated)?;

    // Parse PR reference
    let (owner, repo, number) =
        parse_pr_reference(reference, repo_context).map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
        })?;

    // Create GitHub client with the provided token
    let token = SecretString::from(github_token);
    let client = create_client_with_token(&token).map_err(|e| AptuError::AI {
        message: format!("Failed to create GitHub client: {e}"),
        status: None,
    })?;

    // Post the review
    gh_post_pr_review(&client, &owner, &repo, number, body, event)
        .await
        .map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
        })
}
