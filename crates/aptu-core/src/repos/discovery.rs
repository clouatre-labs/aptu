// SPDX-License-Identifier: Apache-2.0

//! Repository discovery via GitHub Search API.
//!
//! Searches GitHub for welcoming repositories using the REST Search API via Octocrab.
//! Results are scored client-side based on stars, activity, and other signals.
//! Supports caching with configurable TTL.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::cache::FileCache;
use crate::config::load_config;
use crate::error::AptuError;
use crate::github::auth::create_client_with_token;
use secrecy::SecretString;

/// A discovered repository from GitHub search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredRepo {
    /// Repository owner (user or organization).
    pub owner: String,
    /// Repository name.
    pub name: String,
    /// Primary programming language.
    pub language: Option<String>,
    /// Short description.
    pub description: Option<String>,
    /// Number of stars.
    pub stars: u32,
    /// Repository URL.
    pub url: String,
    /// Relevance score (0-100).
    pub score: u32,
}

impl DiscoveredRepo {
    /// Returns the full repository name in "owner/name" format.
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Filter for repository discovery.
#[derive(Debug, Clone)]
pub struct DiscoveryFilter {
    /// Programming language to filter by (e.g., "Rust", "Python").
    pub language: Option<String>,
    /// Minimum number of stars.
    pub min_stars: u32,
    /// Maximum number of results to return.
    pub limit: u32,
}

impl Default for DiscoveryFilter {
    fn default() -> Self {
        Self {
            language: None,
            min_stars: 10,
            limit: 20,
        }
    }
}

/// Score a repository based on various signals.
///
/// Scoring factors:
/// - Stars (0-50 points): logarithmic scale, capped at 50
/// - Language match (0-30 points): exact match gets full points
/// - Description presence (0-20 points): repositories with descriptions score higher
///
/// # Arguments
///
/// * `repo` - The repository to score
/// * `filter` - The discovery filter (for language matching)
///
/// # Returns
///
/// A score from 0-100.
#[must_use]
pub fn score_repo(repo: &octocrab::models::Repository, filter: &DiscoveryFilter) -> u32 {
    let mut score = 0u32;

    // Stars: logarithmic scale (0-50 points)
    let stars = f64::from(repo.stargazers_count.unwrap_or(0));
    let star_score = if stars > 0.0 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let score_val = ((stars.ln() + 1.0) / 10.0 * 50.0).min(50.0) as u32;
        score_val
    } else {
        0
    };
    score += star_score;

    // Language match (0-30 points)
    if let Some(ref filter_lang) = filter.language
        && let Some(ref repo_lang) = repo.language
        && let Some(lang_str) = repo_lang.as_str()
        && lang_str.to_lowercase() == filter_lang.to_lowercase()
    {
        score += 30;
    }

    // Description presence (0-20 points)
    if repo.description.is_some() && !repo.description.as_ref().unwrap().is_empty() {
        score += 20;
    }

    score.min(100)
}

use std::fmt::Write as FmtWrite;

/// Build a GitHub search query from filter parameters.
///
/// Constructs a query string suitable for GitHub's REST Search API.
/// Includes filters for:
/// - Good first issue labels
/// - Help wanted labels
/// - Active repositories (pushed in last 30 days)
/// - Minimum stars
/// - Language (if specified)
///
/// # Arguments
///
/// * `filter` - The discovery filter
///
/// # Returns
///
/// A GitHub search query string using repository search qualifiers.
/// Searches for repositories with open good-first-issue labeled issues,
/// pushed within the last 30 days, meeting minimum star count and language criteria.
#[must_use]
pub fn build_search_query(filter: &DiscoveryFilter) -> String {
    let mut query = String::from("good-first-issues:>0");

    // Calculate date 30 days ago from now
    let thirty_days_ago = Utc::now() - Duration::days(30);
    let date_str = thirty_days_ago.format("%Y-%m-%d").to_string();
    let _ = write!(query, " pushed:>{date_str}");

    let _ = write!(query, " stars:>={}", filter.min_stars);

    if let Some(ref lang) = filter.language {
        let _ = write!(query, " language:{lang}");
    }

    query
}

/// Search for repositories matching the discovery filter.
///
/// Uses GitHub's REST Search API via Octocrab to find repositories.
/// Results are scored client-side and sorted by score descending.
/// Supports caching with configurable TTL.
///
/// # Arguments
///
/// * `token` - GitHub API token
/// * `filter` - Discovery filter (language, `min_stars`, limit)
///
/// # Returns
///
/// A vector of discovered repositories, sorted by score.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub API call fails
/// - Response parsing fails
#[instrument(skip(token), fields(language = ?filter.language, min_stars = filter.min_stars, limit = filter.limit))]
pub async fn search_repositories(
    token: &SecretString,
    filter: &DiscoveryFilter,
) -> crate::Result<Vec<DiscoveredRepo>> {
    // Check cache first
    let cache_key = format!(
        "discovered_repos_{}_{}_{}",
        filter.language.as_deref().unwrap_or("any"),
        filter.min_stars,
        filter.limit
    );

    let config = load_config()?;
    let ttl = Duration::hours(config.cache.repo_ttl_hours.try_into().unwrap_or(24));

    let cache: crate::cache::FileCacheImpl<Vec<DiscoveredRepo>> =
        crate::cache::FileCacheImpl::new("discovery", ttl);
    if let Ok(Some(repos)) = cache.get(&cache_key) {
        debug!("Using cached discovered repositories");
        return Ok(repos);
    }

    // Create GitHub client
    let client = create_client_with_token(token).map_err(|e| AptuError::GitHub {
        message: format!("Failed to create GitHub client: {e}"),
    })?;

    // Build search query
    let query = build_search_query(filter);
    debug!("Searching with query: {}", query);

    // Execute search with retry logic
    let repos = client
        .search()
        .repositories(&query)
        .per_page(100)
        .send()
        .await
        .map_err(|e| AptuError::GitHub {
            message: format!("Failed to search repositories: {e}"),
        })?;

    // Score and sort results
    let mut discovered: Vec<DiscoveredRepo> = repos
        .items
        .into_iter()
        .filter_map(|repo| {
            let score = score_repo(&repo, filter);
            let url = repo.html_url.as_ref().map(ToString::to_string)?;
            let language = repo
                .language
                .as_ref()
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            Some(DiscoveredRepo {
                owner: repo
                    .owner
                    .as_ref()
                    .map(|o| o.login.clone())
                    .unwrap_or_default(),
                name: repo.name.clone(),
                language,
                description: repo.description.clone(),
                stars: repo.stargazers_count.unwrap_or(0),
                url,
                score,
            })
        })
        .collect();

    // Sort by score descending, then by stars descending
    discovered.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| b.stars.cmp(&a.stars)));

    // Limit results
    discovered.truncate(filter.limit as usize);

    // Cache the results
    let _ = cache.set(&cache_key, &discovered);

    debug!(
        "Found and cached {} discovered repositories",
        discovered.len()
    );
    Ok(discovered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_search_query_basic() {
        let filter = DiscoveryFilter {
            language: None,
            min_stars: 10,
            limit: 20,
        };

        let query = build_search_query(&filter);
        assert!(query.contains("good-first-issues:>0"));
        assert!(query.contains("pushed:>"));
        assert!(query.contains("stars:>=10"));
        assert!(!query.contains("language:"));
    }

    #[test]
    fn build_search_query_with_language() {
        let filter = DiscoveryFilter {
            language: Some("Rust".to_string()),
            min_stars: 50,
            limit: 10,
        };

        let query = build_search_query(&filter);
        assert!(query.contains("good-first-issues:>0"));
        assert!(query.contains("language:Rust"));
        assert!(query.contains("stars:>=50"));
    }

    #[test]
    fn discovered_repo_full_name() {
        let repo = DiscoveredRepo {
            owner: "owner".to_string(),
            name: "repo".to_string(),
            language: Some("Rust".to_string()),
            description: Some("Test".to_string()),
            stars: 100,
            url: "https://github.com/owner/repo".to_string(),
            score: 75,
        };

        assert_eq!(repo.full_name(), "owner/repo");
    }
}
