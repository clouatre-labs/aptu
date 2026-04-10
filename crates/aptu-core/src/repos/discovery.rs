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
    if repo.description.as_deref().is_some_and(|d| !d.is_empty()) {
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
    let ttl = Duration::hours(config.cache.repo_ttl_hours);

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

    fn make_repo_for_score(description: Option<String>) -> octocrab::models::Repository {
        let raw = r#"{"id":566109822,"node_id":"R_kgDOIb4mfg","name":"test-repo","full_name":"owner/test-repo","private":false,"owner":{"login":"owner","id":8704475,"node_id":"MDQ6VXNlcjg3MDQ0NzU=","avatar_url":"https://avatars.githubusercontent.com/u/8704475?v=4","gravatar_id":"","url":"https://api.github.com/users/owner","html_url":"https://github.com/owner","followers_url":"https://api.github.com/users/owner/followers","following_url":"https://api.github.com/users/owner/following{/other_user}","gists_url":"https://api.github.com/users/owner/gists{/gist_id}","starred_url":"https://api.github.com/users/owner/starred{/owner}{/repo}","subscriptions_url":"https://api.github.com/users/owner/subscriptions","organizations_url":"https://api.github.com/users/owner/orgs","repos_url":"https://api.github.com/users/owner/repos","events_url":"https://api.github.com/users/owner/events{/privacy}","received_events_url":"https://api.github.com/users/owner/received_events","type":"User","site_admin":false},"html_url":"https://github.com/owner/test-repo","description":null,"fork":false,"url":"https://api.github.com/repos/owner/test-repo","forks_url":"https://api.github.com/repos/owner/test-repo/forks","keys_url":"https://api.github.com/repos/owner/test-repo/keys{/key_id}","collaborators_url":"https://api.github.com/repos/owner/test-repo/collaborators{/collaborator}","teams_url":"https://api.github.com/repos/owner/test-repo/teams","hooks_url":"https://api.github.com/repos/owner/test-repo/hooks","issue_events_url":"https://api.github.com/repos/owner/test-repo/issues/events{/number}","events_url":"https://api.github.com/repos/owner/test-repo/events","assignees_url":"https://api.github.com/repos/owner/test-repo/assignees{/user}","branches_url":"https://api.github.com/repos/owner/test-repo/branches{/branch}","tags_url":"https://api.github.com/repos/owner/test-repo/tags","blobs_url":"https://api.github.com/repos/owner/test-repo/git/blobs{/sha}","git_tags_url":"https://api.github.com/repos/owner/test-repo/git/tags{/sha}","git_refs_url":"https://api.github.com/repos/owner/test-repo/git/refs{/sha}","trees_url":"https://api.github.com/repos/owner/test-repo/git/trees{/sha}","statuses_url":"https://api.github.com/repos/owner/test-repo/statuses/{sha}","languages_url":"https://api.github.com/repos/owner/test-repo/languages","stargazers_url":"https://api.github.com/repos/owner/test-repo/stargazers","contributors_url":"https://api.github.com/repos/owner/test-repo/contributors","subscribers_url":"https://api.github.com/repos/owner/test-repo/subscribers","subscription_url":"https://api.github.com/repos/owner/test-repo/subscription","commits_url":"https://api.github.com/repos/owner/test-repo/commits{/sha}","git_commits_url":"https://api.github.com/repos/owner/test-repo/git/commits{/sha}","comments_url":"https://api.github.com/repos/owner/test-repo/comments{/number}","issue_comment_url":"https://api.github.com/repos/owner/test-repo/issues/comments{/number}","contents_url":"https://api.github.com/repos/owner/test-repo/contents/{+path}","compare_url":"https://api.github.com/repos/owner/test-repo/compare/{base}...{head}","merges_url":"https://api.github.com/repos/owner/test-repo/merges","archive_url":"https://api.github.com/repos/owner/test-repo/{archive_format}{/ref}","downloads_url":"https://api.github.com/repos/owner/test-repo/downloads","issues_url":"https://api.github.com/repos/owner/test-repo/issues{/number}","pulls_url":"https://api.github.com/repos/owner/test-repo/pulls{/number}","milestones_url":"https://api.github.com/repos/owner/test-repo/milestones{/number}","notifications_url":"https://api.github.com/repos/owner/test-repo/notifications{?since,all,participating}","labels_url":"https://api.github.com/repos/owner/test-repo/labels{/name}","releases_url":"https://api.github.com/repos/owner/test-repo/releases{/id}","deployments_url":"https://api.github.com/repos/owner/test-repo/deployments","created_at":"2022-11-15T01:30:03Z","updated_at":"2022-11-14T09:34:10Z","pushed_at":"2022-11-15T07:52:50Z","git_url":"git://github.com/owner/test-repo.git","ssh_url":"git@github.com:owner/test-repo.git","clone_url":"https://github.com/owner/test-repo.git","svn_url":"https://github.com/owner/test-repo","size":0,"stargazers_count":0,"watchers_count":0,"has_issues":true,"has_projects":true,"has_downloads":true,"has_wiki":true,"has_pages":false,"has_discussions":false,"forks_count":0,"archived":false,"disabled":false,"open_issues_count":0,"allow_forking":true,"is_template":false,"web_commit_signoff_required":false,"topics":[],"visibility":"public","forks":0,"open_issues":0,"watchers":0,"default_branch":"main"}"#;
        let mut repo: octocrab::models::Repository =
            serde_json::from_str(raw).expect("valid repo JSON");
        repo.description = description;
        repo
    }

    #[test]
    fn test_score_repo_description_adds_points() {
        let filter = DiscoveryFilter {
            language: None,
            min_stars: 0,
            limit: 10,
        };

        let with_desc = make_repo_for_score(Some("A useful crate".to_string()));
        let without_desc = make_repo_for_score(None);

        let score_with = score_repo(&with_desc, &filter);
        let score_without = score_repo(&without_desc, &filter);

        assert_eq!(score_with - score_without, 20);
    }

    #[test]
    fn test_score_repo_description_none_no_panic() {
        let filter = DiscoveryFilter {
            language: None,
            min_stars: 0,
            limit: 10,
        };

        let repo = make_repo_for_score(None);
        // Must not panic
        let score = score_repo(&repo, &filter);
        assert!(score < 100);
    }
}
