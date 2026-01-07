// SPDX-License-Identifier: Apache-2.0

//! Platform-agnostic facade functions for FFI and CLI integration.
//!
//! This module provides high-level functions that abstract away the complexity
//! of credential resolution, API client creation, and data transformation.
//! Each platform (CLI, iOS, MCP) implements `TokenProvider` and calls these
//! functions with their own credential source.

use chrono::Duration;
use tracing::{debug, info, instrument, warn};

use crate::ai::provider::MAX_LABELS;
use crate::ai::registry::get_provider;
use crate::ai::types::{CreateIssueResponse, PrDetails, ReviewEvent, TriageResponse};
use crate::ai::{AiClient, AiProvider, AiResponse, types::IssueDetails};
use crate::auth::TokenProvider;
use crate::cache::{self, CacheEntry};
use crate::config::{AiConfig, TaskType, load_config};
use crate::error::AptuError;
use crate::github::auth::{create_client_from_provider, create_client_with_token};
use crate::github::graphql::{
    IssueNode, fetch_issue_with_repo_context, fetch_issues as gh_fetch_issues,
};
use crate::github::issues::{create_issue as gh_create_issue, filter_labels_by_relevance};
use crate::github::pulls::{fetch_pr_details, post_pr_review as gh_post_pr_review};
use crate::repos::{self, CuratedRepo};
use crate::retry::is_retryable_anyhow;
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
                .map_err(|e| AptuError::GitHub {
                    message: format!("Failed to fetch issues: {e}"),
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
            .map_err(|e| AptuError::GitHub {
                message: format!("Failed to fetch issues: {e}"),
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
#[instrument]
pub async fn list_repos(filter: repos::RepoFilter) -> crate::Result<Vec<CuratedRepo>> {
    repos::fetch_all(filter).await
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
#[instrument(skip(provider), fields(language = ?filter.language, min_stars = filter.min_stars, limit = filter.limit))]
pub async fn discover_repos(
    provider: &dyn TokenProvider,
    filter: repos::discovery::DiscoveryFilter,
) -> crate::Result<Vec<repos::discovery::DiscoveredRepo>> {
    let token = provider.github_token().ok_or(AptuError::NotAuthenticated)?;
    let token = SecretString::from(token);
    repos::discovery::search_repositories(&token, &filter).await
}

/// Generic helper function to try AI operations with fallback chain.
///
/// Attempts an AI operation with the primary provider first. If the primary
/// provider fails with a non-retryable error, iterates through the fallback chain.
///
/// # Arguments
///
/// * `provider` - Token provider for AI credentials
/// * `primary_provider` - Primary AI provider name
/// * `model_name` - Model name to use
/// * `ai_config` - AI configuration including fallback chain
/// * `operation` - Async closure that performs the AI operation
///
/// # Returns
///
/// Validates a model for a given provider, converting registry errors to `AptuError`.
fn validate_provider_model(provider: &str, model: &str) -> crate::Result<()> {
    // Simple static validation: check if provider exists
    if crate::ai::registry::get_provider(provider).is_none() {
        return Err(AptuError::ModelRegistry {
            message: format!("Provider not found: {provider}"),
        });
    }

    // For now, we allow any model ID (permissive fallback)
    // Unknown models will log a warning but won't fail validation
    tracing::debug!(provider = provider, model = model, "Validating model");
    Ok(())
}

/// Result of the AI operation, or error if all providers fail.
async fn try_with_fallback<T, F, Fut>(
    provider: &dyn TokenProvider,
    primary_provider: &str,
    model_name: &str,
    ai_config: &AiConfig,
    operation: F,
) -> crate::Result<T>
where
    F: Fn(AiClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let api_key = provider.ai_api_key(primary_provider).ok_or_else(|| {
        let env_var = get_provider(primary_provider).map_or("API_KEY", |p| p.api_key_env);
        AptuError::AiProviderNotAuthenticated {
            provider: primary_provider.to_string(),
            env_var: env_var.to_string(),
        }
    })?;

    if ai_config.validation_enabled {
        validate_provider_model(primary_provider, model_name)?;
    }

    let ai_client = AiClient::with_api_key(primary_provider, api_key, model_name, ai_config)
        .map_err(|e| AptuError::AI {
            message: e.to_string(),
            status: None,
            provider: primary_provider.to_string(),
        })?;

    match operation(ai_client).await {
        Ok(response) => return Ok(response),
        Err(e) => {
            if is_retryable_anyhow(&e) {
                return Err(AptuError::AI {
                    message: e.to_string(),
                    status: None,
                    provider: primary_provider.to_string(),
                });
            }
            warn!(
                primary_provider = primary_provider,
                error = %e,
                "Primary provider failed with non-retryable error, trying fallback chain"
            );
        }
    }

    if let Some(fallback_config) = &ai_config.fallback {
        for entry in &fallback_config.chain {
            warn!(
                fallback_provider = entry.provider,
                "Attempting fallback provider"
            );

            let Some(api_key) = provider.ai_api_key(&entry.provider) else {
                warn!(
                    fallback_provider = entry.provider,
                    "No API key available for fallback provider"
                );
                continue;
            };

            let fallback_model = entry.model.as_deref().unwrap_or(model_name);

            if ai_config.validation_enabled
                && validate_provider_model(&entry.provider, fallback_model).is_err()
            {
                warn!(
                    fallback_provider = entry.provider,
                    fallback_model = fallback_model,
                    "Fallback provider model validation failed, continuing to next provider"
                );
                continue;
            }

            let Ok(ai_client) =
                AiClient::with_api_key(&entry.provider, api_key, fallback_model, ai_config)
            else {
                warn!(
                    fallback_provider = entry.provider,
                    "Failed to create AI client for fallback provider"
                );
                continue;
            };

            match operation(ai_client).await {
                Ok(response) => {
                    info!(
                        fallback_provider = entry.provider,
                        "Successfully completed operation with fallback provider"
                    );
                    return Ok(response);
                }
                Err(e) => {
                    warn!(
                        fallback_provider = entry.provider,
                        error = %e,
                        "Fallback provider failed"
                    );
                }
            }
        }
    }

    Err(AptuError::AI {
        message: "All AI providers failed (primary and fallback chain)".to_string(),
        status: None,
        provider: primary_provider.to_string(),
    })
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
    ai_config: &AiConfig,
) -> crate::Result<AiResponse> {
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

    // Resolve task-specific provider and model
    let (provider_name, model_name) = ai_config.resolve_for_task(TaskType::Triage);

    // Use fallback chain if configured
    try_with_fallback(provider, &provider_name, &model_name, ai_config, |client| {
        let issue = issue_mut.clone();
        async move { client.analyze_issue(&issue).await }
    })
    .await
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
/// * `ai_config` - AI configuration (provider, model, etc.)
///
/// # Returns
///
/// Tuple of (`PrDetails`, `PrReviewResponse`) with PR info and AI review.
///
/// # Errors
///
/// Fetches PR details for review without AI analysis.
///
/// This function handles credential resolution and GitHub API calls,
/// allowing platforms to display PR metadata before starting AI analysis.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `reference` - PR reference (URL, owner/repo#number, or number)
/// * `repo_context` - Optional repository context for bare numbers
///
/// # Returns
///
/// PR details including title, body, files, and labels.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - PR cannot be fetched
#[instrument(skip(provider), fields(reference = %reference))]
pub async fn fetch_pr_for_review(
    provider: &dyn TokenProvider,
    reference: &str,
    repo_context: Option<&str>,
) -> crate::Result<PrDetails> {
    use crate::github::pulls::parse_pr_reference;

    // Parse PR reference
    let (owner, repo, number) =
        parse_pr_reference(reference, repo_context).map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Fetch PR details
    fetch_pr_details(&client, &owner, &repo, number)
        .await
        .map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })
}

/// Analyzes PR details with AI to generate a review.
///
/// This function takes pre-fetched PR details and performs AI analysis.
/// It should be called after `fetch_pr_for_review()` to allow intermediate display.
///
/// # Arguments
///
/// * `provider` - Token provider for AI credentials
/// * `pr_details` - PR details from `fetch_pr_for_review()`
/// * `ai_config` - AI configuration
///
/// # Returns
///
/// Tuple of (review response, AI stats).
///
/// # Errors
///
/// Returns an error if:
/// - AI provider token is not available from the provider
/// - AI API call fails
#[instrument(skip(provider, pr_details), fields(number = pr_details.number))]
pub async fn analyze_pr(
    provider: &dyn TokenProvider,
    pr_details: &PrDetails,
    ai_config: &AiConfig,
) -> crate::Result<(crate::ai::types::PrReviewResponse, crate::history::AiStats)> {
    // Resolve task-specific provider and model
    let (provider_name, model_name) = ai_config.resolve_for_task(TaskType::Review);

    // Use fallback chain if configured
    try_with_fallback(provider, &provider_name, &model_name, ai_config, |client| {
        let pr = pr_details.clone();
        async move { client.review_pr(&pr).await }
    })
    .await
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

    // Parse PR reference
    let (owner, repo, number) =
        parse_pr_reference(reference, repo_context).map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Post the review
    gh_post_pr_review(&client, &owner, &repo, number, body, event)
        .await
        .map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })
}

/// Auto-label a pull request based on conventional commit prefix and file paths.
///
/// Fetches PR details, extracts labels from title and changed files,
/// and applies them to the PR. Optionally previews without applying.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `reference` - PR reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
/// * `dry_run` - If true, preview labels without applying
///
/// # Returns
///
/// Tuple of (`pr_number`, `pr_title`, `pr_url`, `labels`).
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - PR cannot be parsed or found
/// - API call fails
#[instrument(skip(provider), fields(reference = %reference))]
pub async fn label_pr(
    provider: &dyn TokenProvider,
    reference: &str,
    repo_context: Option<&str>,
    dry_run: bool,
    ai_config: &AiConfig,
) -> crate::Result<(u64, String, String, Vec<String>)> {
    use crate::github::issues::apply_labels_to_number;
    use crate::github::pulls::{fetch_pr_details, labels_from_pr_metadata, parse_pr_reference};

    // Parse PR reference
    let (owner, repo, number) =
        parse_pr_reference(reference, repo_context).map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Fetch PR details
    let pr_details = fetch_pr_details(&client, &owner, &repo, number)
        .await
        .map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

    // Extract labels from PR metadata (deterministic approach)
    let file_paths: Vec<String> = pr_details
        .files
        .iter()
        .map(|f| f.filename.clone())
        .collect();
    let mut labels = labels_from_pr_metadata(&pr_details.title, &file_paths);

    // If no labels found, try AI fallback
    if labels.is_empty() {
        // Resolve task-specific provider and model for Create task
        let (provider_name, model_name) = ai_config.resolve_for_task(TaskType::Create);

        // Get API key from provider using the resolved provider name
        if let Some(api_key) = provider.ai_api_key(&provider_name) {
            // Create AI client with resolved provider and model
            if let Ok(ai_client) =
                crate::ai::AiClient::with_api_key(&provider_name, api_key, &model_name, ai_config)
            {
                match ai_client
                    .suggest_pr_labels(&pr_details.title, &pr_details.body, &file_paths)
                    .await
                {
                    Ok((ai_labels, _stats)) => {
                        labels = ai_labels;
                        debug!("AI fallback provided {} labels", labels.len());
                    }
                    Err(e) => {
                        debug!("AI fallback failed: {}", e);
                        // Continue without labels rather than failing
                    }
                }
            }
        }
    }

    // Apply labels if not dry-run
    if !dry_run && !labels.is_empty() {
        apply_labels_to_number(&client, &owner, &repo, number, &labels)
            .await
            .map_err(|e| AptuError::GitHub {
                message: e.to_string(),
            })?;
    }

    Ok((number, pr_details.title, pr_details.url, labels))
}

/// Fetches an issue for triage analysis.
///
/// Parses the issue reference, checks authentication, and fetches issue details
/// including labels, milestones, and repository context.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `reference` - Issue reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
///
/// # Returns
///
/// Issue details including title, body, labels, comments, and available labels/milestones.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - Issue reference cannot be parsed
/// - GitHub API call fails
#[allow(clippy::too_many_lines)]
#[instrument(skip(provider), fields(reference = %reference))]
pub async fn fetch_issue_for_triage(
    provider: &dyn TokenProvider,
    reference: &str,
    repo_context: Option<&str>,
) -> crate::Result<IssueDetails> {
    // Parse the issue reference
    let (owner, repo, number) =
        crate::github::issues::parse_issue_reference(reference, repo_context).map_err(|e| {
            AptuError::GitHub {
                message: e.to_string(),
            }
        })?;

    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Fetch issue with repository context (labels, milestones) in a single GraphQL call
    let (issue_node, repo_data) = fetch_issue_with_repo_context(&client, &owner, &repo, number)
        .await
        .map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

    // Convert GraphQL response to IssueDetails
    let labels: Vec<String> = issue_node
        .labels
        .nodes
        .iter()
        .map(|label| label.name.clone())
        .collect();

    let comments: Vec<crate::ai::types::IssueComment> = issue_node
        .comments
        .nodes
        .iter()
        .map(|comment| crate::ai::types::IssueComment {
            author: comment.author.login.clone(),
            body: comment.body.clone(),
        })
        .collect();

    let available_labels: Vec<crate::ai::types::RepoLabel> = repo_data
        .labels
        .nodes
        .iter()
        .map(|label| crate::ai::types::RepoLabel {
            name: label.name.clone(),
            description: String::new(),
            color: String::new(),
        })
        .collect();

    let available_milestones: Vec<crate::ai::types::RepoMilestone> = repo_data
        .milestones
        .nodes
        .iter()
        .map(|milestone| crate::ai::types::RepoMilestone {
            number: milestone.number,
            title: milestone.title.clone(),
            description: String::new(),
        })
        .collect();

    let mut issue_details = IssueDetails::builder()
        .owner(owner.clone())
        .repo(repo.clone())
        .number(number)
        .title(issue_node.title.clone())
        .body(issue_node.body.clone().unwrap_or_default())
        .labels(labels)
        .comments(comments)
        .url(issue_node.url.clone())
        .available_labels(available_labels)
        .available_milestones(available_milestones)
        .build();

    // Populate optional fields from issue_node
    issue_details.author = issue_node.author.as_ref().map(|a| a.login.clone());
    issue_details.created_at = Some(issue_node.created_at.clone());
    issue_details.updated_at = Some(issue_node.updated_at.clone());

    // Extract keywords and language for parallel calls
    let keywords = crate::github::issues::extract_keywords(&issue_details.title);
    let language = repo_data
        .primary_language
        .as_ref()
        .map_or("unknown", |l| l.name.as_str())
        .to_string();

    // Run search and tree fetch in parallel
    let (search_result, tree_result) = tokio::join!(
        crate::github::issues::search_related_issues(
            &client,
            &owner,
            &repo,
            &issue_details.title,
            number
        ),
        crate::github::issues::fetch_repo_tree(&client, &owner, &repo, &language, &keywords)
    );

    // Handle search results
    match search_result {
        Ok(related) => {
            issue_details.repo_context = related;
            debug!(
                related_count = issue_details.repo_context.len(),
                "Found related issues"
            );
        }
        Err(e) => {
            debug!(error = %e, "Failed to search for related issues, continuing without context");
        }
    }

    // Handle tree results
    match tree_result {
        Ok(tree) => {
            issue_details.repo_tree = tree;
            debug!(
                tree_count = issue_details.repo_tree.len(),
                "Fetched repository tree"
            );
        }
        Err(e) => {
            debug!(error = %e, "Failed to fetch repository tree, continuing without context");
        }
    }

    debug!(issue_number = number, "Issue fetched successfully");
    Ok(issue_details)
}

/// Posts a triage comment to GitHub.
///
/// Renders the triage response as markdown and posts it as a comment on the issue.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `issue_details` - Issue details (owner, repo, number)
/// * `triage` - Triage response to post
///
/// # Returns
///
/// The URL of the posted comment.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - GitHub API call fails
#[instrument(skip(provider, triage), fields(owner = %issue_details.owner, repo = %issue_details.repo, number = issue_details.number))]
pub async fn post_triage_comment(
    provider: &dyn TokenProvider,
    issue_details: &IssueDetails,
    triage: &TriageResponse,
) -> crate::Result<String> {
    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Render markdown and post comment
    let comment_body = crate::triage::render_triage_markdown(triage);
    let comment_url = crate::github::issues::post_comment(
        &client,
        &issue_details.owner,
        &issue_details.repo,
        issue_details.number,
        &comment_body,
    )
    .await
    .map_err(|e| AptuError::GitHub {
        message: e.to_string(),
    })?;

    debug!(comment_url = %comment_url, "Triage comment posted");
    Ok(comment_url)
}

/// Applies AI-suggested labels and milestone to an issue.
///
/// Labels are applied additively: existing labels are preserved and AI-suggested labels
/// are merged in. Priority labels (p1/p2/p3) defer to existing human judgment.
/// Milestones are only set if the issue doesn't already have one.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `issue_details` - Issue details including available labels and milestones
/// * `triage` - AI triage response with suggestions
///
/// # Returns
///
/// Result of applying labels and milestone.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - GitHub API call fails
#[instrument(skip(provider, triage), fields(owner = %issue_details.owner, repo = %issue_details.repo, number = issue_details.number))]
pub async fn apply_triage_labels(
    provider: &dyn TokenProvider,
    issue_details: &IssueDetails,
    triage: &TriageResponse,
) -> crate::Result<crate::github::issues::ApplyResult> {
    debug!("Applying labels and milestone to issue");

    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Call the update function with validation
    let result = crate::github::issues::update_issue_labels_and_milestone(
        &client,
        &issue_details.owner,
        &issue_details.repo,
        issue_details.number,
        &issue_details.labels,
        &triage.suggested_labels,
        issue_details.milestone.as_deref(),
        triage.suggested_milestone.as_deref(),
        &issue_details.available_labels,
        &issue_details.available_milestones,
    )
    .await
    .map_err(|e| AptuError::GitHub {
        message: e.to_string(),
    })?;

    info!(
        labels = ?result.applied_labels,
        milestone = ?result.applied_milestone,
        warnings = ?result.warnings,
        "Labels and milestone applied"
    );

    Ok(result)
}

/// Generate AI-curated release notes from PRs between git tags.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `from_tag` - Starting tag (or None for latest)
/// * `to_tag` - Ending tag (or None for HEAD)
///
/// # Returns
///
/// Structured release notes with theme, highlights, and categorized changes.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available
/// - GitHub API calls fail
/// - AI response parsing fails
///
/// Helper to get a reference from the previous tag or fall back to root commit.
///
/// This helper encapsulates the common pattern of trying to get the previous tag
/// (before the target tag), and if no previous tag exists, falling back to the root
/// commit for first release scenarios.
///
/// # Arguments
///
/// * `gh_client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `to_ref` - The target tag to find the predecessor for
///
/// # Returns
///
/// A commit SHA or tag name to use as a reference.
///
/// # Errors
///
/// Returns an error if both tag and root commit fetches fail.
async fn get_from_ref_or_root(
    gh_client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    to_ref: &str,
) -> Result<String, AptuError> {
    // Try to find the previous tag before the target tag
    let previous_tag_opt =
        crate::github::releases::get_previous_tag(gh_client, owner, repo, to_ref)
            .await
            .map_err(|e| AptuError::GitHub {
                message: e.to_string(),
            })?;

    if let Some((tag, _)) = previous_tag_opt {
        Ok(tag)
    } else {
        // No previous tag exists, use root commit for first release
        tracing::info!(
            "No previous tag found before {}, using root commit for first release",
            to_ref
        );
        crate::github::releases::get_root_commit(gh_client, owner, repo)
            .await
            .map_err(|e| AptuError::GitHub {
                message: e.to_string(),
            })
    }
}

/// Generate AI-curated release notes from PRs between git tags.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `from_tag` - Starting tag (or None for latest)
/// * `to_tag` - Ending tag (or None for HEAD)
///
/// # Returns
///
/// Structured release notes with theme, highlights, and categorized changes.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available
/// - GitHub API calls fail
/// - AI response parsing fails
#[instrument(skip(provider))]
pub async fn generate_release_notes(
    provider: &dyn TokenProvider,
    owner: &str,
    repo: &str,
    from_tag: Option<&str>,
    to_tag: Option<&str>,
) -> Result<crate::ai::types::ReleaseNotesResponse, AptuError> {
    let token = provider.github_token().ok_or_else(|| AptuError::GitHub {
        message: "GitHub token not available".to_string(),
    })?;

    let gh_client = create_client_with_token(&token).map_err(|e| AptuError::GitHub {
        message: e.to_string(),
    })?;

    // Load AI config
    let config = load_config().map_err(|e| AptuError::Config {
        message: e.to_string(),
    })?;

    // Create AI client
    let ai_client = AiClient::new(&config.ai.provider, &config.ai).map_err(|e| AptuError::AI {
        message: e.to_string(),
        status: None,
        provider: config.ai.provider.clone(),
    })?;

    // Determine tags to use
    let (from_ref, to_ref) = if let (Some(from), Some(to)) = (from_tag, to_tag) {
        (from.to_string(), to.to_string())
    } else if let Some(to) = to_tag {
        // Get previous tag before to_ref, or root commit if no previous tag exists
        let from_ref = get_from_ref_or_root(&gh_client, owner, repo, to).await?;
        (from_ref, to.to_string())
    } else if let Some(from) = from_tag {
        // Use HEAD as to_ref
        (from.to_string(), "HEAD".to_string())
    } else {
        // Get latest tag and use HEAD, or root commit if no tags exist
        // For this case, we need to get the latest tag first, then find its predecessor
        let latest_tag_opt = crate::github::releases::get_latest_tag(&gh_client, owner, repo)
            .await
            .map_err(|e| AptuError::GitHub {
                message: e.to_string(),
            })?;

        let to_ref = if let Some((tag, _)) = latest_tag_opt {
            tag
        } else {
            "HEAD".to_string()
        };

        let from_ref = get_from_ref_or_root(&gh_client, owner, repo, &to_ref).await?;
        (from_ref, to_ref)
    };

    // Fetch PRs between tags
    let prs = crate::github::releases::fetch_prs_between_refs(
        &gh_client, owner, repo, &from_ref, &to_ref,
    )
    .await
    .map_err(|e| AptuError::GitHub {
        message: e.to_string(),
    })?;

    if prs.is_empty() {
        return Err(AptuError::GitHub {
            message: "No merged PRs found between the specified tags".to_string(),
        });
    }

    // Generate release notes via AI
    let version = crate::github::releases::parse_tag_reference(&to_ref);
    let (response, _ai_stats) = ai_client
        .generate_release_notes(prs, &version)
        .await
        .map_err(|e: anyhow::Error| AptuError::AI {
            message: e.to_string(),
            status: None,
            provider: config.ai.provider.clone(),
        })?;

    info!(
        theme = ?response.theme,
        highlights_count = response.highlights.len(),
        contributors_count = response.contributors.len(),
        "Release notes generated"
    );

    Ok(response)
}

/// Post release notes to GitHub.
///
/// Creates or updates a release on GitHub with the provided release notes body.
/// If the release already exists, it will be updated. Otherwise, a new release is created.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `tag` - The tag name for the release
/// * `body` - The release notes body
///
/// # Returns
///
/// The URL of the created or updated release.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available
/// - GitHub API call fails
#[instrument(skip(provider))]
pub async fn post_release_notes(
    provider: &dyn TokenProvider,
    owner: &str,
    repo: &str,
    tag: &str,
    body: &str,
) -> Result<String, AptuError> {
    let token = provider.github_token().ok_or_else(|| AptuError::GitHub {
        message: "GitHub token not available".to_string(),
    })?;

    let gh_client = create_client_with_token(&token).map_err(|e| AptuError::GitHub {
        message: e.to_string(),
    })?;

    crate::github::releases::post_release_notes(&gh_client, owner, repo, tag, body)
        .await
        .map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use crate::config::{FallbackConfig, FallbackEntry};

    #[test]
    fn test_fallback_chain_config_structure() {
        // Test that fallback chain config structure is correct
        let fallback_config = FallbackConfig {
            chain: vec![
                FallbackEntry {
                    provider: "openrouter".to_string(),
                    model: None,
                },
                FallbackEntry {
                    provider: "anthropic".to_string(),
                    model: Some("claude-haiku-4.5".to_string()),
                },
            ],
        };

        assert_eq!(fallback_config.chain.len(), 2);
        assert_eq!(fallback_config.chain[0].provider, "openrouter");
        assert_eq!(fallback_config.chain[0].model, None);
        assert_eq!(fallback_config.chain[1].provider, "anthropic");
        assert_eq!(
            fallback_config.chain[1].model,
            Some("claude-haiku-4.5".to_string())
        );
    }

    #[test]
    fn test_fallback_chain_empty() {
        // Test that empty fallback chain is valid
        let fallback_config = FallbackConfig { chain: vec![] };

        assert_eq!(fallback_config.chain.len(), 0);
    }

    #[test]
    fn test_fallback_chain_single_provider() {
        // Test that single provider fallback chain is valid
        let fallback_config = FallbackConfig {
            chain: vec![FallbackEntry {
                provider: "openrouter".to_string(),
                model: None,
            }],
        };

        assert_eq!(fallback_config.chain.len(), 1);
        assert_eq!(fallback_config.chain[0].provider, "openrouter");
    }
}

#[allow(clippy::items_after_test_module)]
/// Formats a GitHub issue with AI assistance.
///
/// This function takes raw issue title and body, and uses AI to format them
/// according to project conventions. Returns formatted title, body, and suggested labels.
///
/// This is the first step of the two-step issue creation process. Use `post_issue()`
/// to post the formatted issue to GitHub.
///
/// # Arguments
///
/// * `provider` - Token provider for AI provider credentials
/// * `title` - Raw issue title
/// * `body` - Raw issue body
/// * `repo` - Repository name (owner/repo format) for context
/// * `ai_config` - AI configuration (provider, model, etc.)
///
/// # Returns
///
/// `CreateIssueResponse` with formatted title, body, and suggested labels.
///
/// # Errors
///
/// Returns an error if:
/// - AI provider token is not available from the provider
/// - AI API call fails
/// - Response parsing fails
#[instrument(skip(provider, ai_config), fields(repo = %repo))]
pub async fn format_issue(
    provider: &dyn TokenProvider,
    title: &str,
    body: &str,
    repo: &str,
    ai_config: &AiConfig,
) -> crate::Result<CreateIssueResponse> {
    // Resolve task-specific provider and model
    let (provider_name, model_name) = ai_config.resolve_for_task(TaskType::Create);

    // Use fallback chain if configured
    try_with_fallback(provider, &provider_name, &model_name, ai_config, |client| {
        let title = title.to_string();
        let body = body.to_string();
        let repo = repo.to_string();
        async move {
            let (response, _stats) = client.create_issue(&title, &body, &repo).await?;
            Ok(response)
        }
    })
    .await
}

/// Posts a formatted issue to GitHub.
///
/// This function takes formatted issue content and posts it to GitHub.
/// It is the second step of the two-step issue creation process.
/// Use `format_issue()` first to format the issue content.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `title` - Formatted issue title
/// * `body` - Formatted issue body
///
/// # Returns
///
/// Tuple of (`issue_url`, `issue_number`).
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - GitHub API call fails
#[instrument(skip(provider), fields(owner = %owner, repo = %repo))]
pub async fn post_issue(
    provider: &dyn TokenProvider,
    owner: &str,
    repo: &str,
    title: &str,
    body: &str,
) -> crate::Result<(String, u64)> {
    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Post issue to GitHub
    gh_create_issue(&client, owner, repo, title, body)
        .await
        .map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })
}
/// Lists available models from a provider API with caching.
///
/// This function fetches the list of available models from a provider's API,
/// with automatic caching and TTL validation. If the cache is valid, it returns
/// cached data. Otherwise, it fetches from the API and updates the cache.
///
/// # Arguments
///
/// * `provider` - Token provider for API credentials
/// * `provider_name` - Name of the provider (e.g., "openrouter", "gemini")
///
/// # Returns
///
/// A vector of `ModelInfo` structs with available models.
///
/// # Errors
///
/// Returns an error if:
/// - Provider is not found
/// - API request fails
/// - Response parsing fails
#[instrument(skip(provider), fields(provider_name))]
pub async fn list_models(
    provider: &dyn TokenProvider,
    provider_name: &str,
) -> crate::Result<Vec<crate::ai::registry::CachedModel>> {
    use crate::ai::registry::{CachedModelRegistry, ModelRegistry};
    use crate::cache::cache_dir;

    let cache_dir = cache_dir();
    let registry = CachedModelRegistry::new(cache_dir, 86400, provider); // 24h TTL

    registry
        .list_models(provider_name)
        .await
        .map_err(|e| AptuError::ModelRegistry {
            message: format!("Failed to list models: {e}"),
        })
}

/// Validates if a model exists for a provider.
///
/// This function checks if a specific model identifier is available for a provider,
/// using the cached model registry with automatic caching.
///
/// # Arguments
///
/// * `provider` - Token provider for API credentials
/// * `provider_name` - Name of the provider (e.g., "openrouter", "gemini")
/// * `model_id` - Model identifier to validate
///
/// # Returns
///
/// `true` if the model exists, `false` otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - Provider is not found
/// - API request fails
/// - Response parsing fails
#[instrument(skip(provider), fields(provider_name, model_id))]
pub async fn validate_model(
    provider: &dyn TokenProvider,
    provider_name: &str,
    model_id: &str,
) -> crate::Result<bool> {
    use crate::ai::registry::{CachedModelRegistry, ModelRegistry};
    use crate::cache::cache_dir;

    let cache_dir = cache_dir();
    let registry = CachedModelRegistry::new(cache_dir, 86400, provider); // 24h TTL

    registry
        .model_exists(provider_name, model_id)
        .await
        .map_err(|e| AptuError::ModelRegistry {
            message: format!("Failed to validate model: {e}"),
        })
}
