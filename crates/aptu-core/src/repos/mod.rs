// SPDX-License-Identifier: Apache-2.0

//! Curated and custom repository management for Aptu.
//!
//! Repositories can come from two sources:
//! - Curated: fetched from a remote JSON file with TTL-based caching
//! - Custom: stored locally in TOML format at `~/.config/aptu/repos.toml`
//!
//! The curated list contains repositories known to be:
//! - Active (commits in last 30 days)
//! - Welcoming (good first issue labels exist)
//! - Responsive (maintainers reply within 1 week)

pub mod custom;
pub mod discovery;

use chrono::Duration;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::cache::FileCache;
use crate::config::load_config;

/// Embedded curated repositories as fallback when network fetch fails.
const EMBEDDED_REPOS: &str = include_str!("../../data/curated-repos.json");

/// A curated repository for contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuratedRepo {
    /// Repository owner (user or organization).
    pub owner: String,
    /// Repository name.
    pub name: String,
    /// Primary programming language.
    pub language: String,
    /// Short description.
    pub description: String,
}

impl CuratedRepo {
    /// Returns the full repository name in "owner/name" format.
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Parse embedded curated repositories from the compiled-in JSON.
///
/// # Returns
///
/// A vector of `CuratedRepo` structs parsed from the embedded JSON.
///
/// # Panics
///
/// Panics if the embedded JSON is malformed (should never happen in production).
fn embedded_defaults() -> Vec<CuratedRepo> {
    serde_json::from_str(EMBEDDED_REPOS).expect("embedded repos JSON is valid")
}

/// Fetch curated repositories from remote URL with TTL-based caching.
///
/// Fetches the curated repository list from a remote JSON file
/// (configured via `cache.curated_repos_url`), caching the result with a TTL
/// based on `cache.repo_ttl_hours`.
///
/// If the network fetch fails, falls back to embedded defaults with a warning.
///
/// # Returns
///
/// A vector of `CuratedRepo` structs.
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be loaded
pub async fn fetch() -> crate::Result<Vec<CuratedRepo>> {
    let config = load_config()?;
    let url = &config.cache.curated_repos_url;
    let ttl = Duration::hours(config.cache.repo_ttl_hours);

    // Try cache first
    let cache: crate::cache::FileCacheImpl<Vec<CuratedRepo>> =
        crate::cache::FileCacheImpl::new("repos", ttl);
    if let Ok(Some(repos)) = cache.get("curated_repos") {
        debug!("Using cached curated repositories");
        return Ok(repos);
    }

    // Fetch from remote
    debug!("Fetching curated repositories from {}", url);
    let repos = if let Ok(repos) = reqwest::Client::new().get(url).send().await?.json().await {
        repos
    } else {
        warn!("Failed to fetch remote curated repositories, using embedded defaults");
        embedded_defaults()
    };

    // Cache the result
    let _ = cache.set("curated_repos", &repos);
    debug!("Fetched and cached {} curated repositories", repos.len());

    Ok(repos)
}

/// Repository filter for fetching repositories.
#[derive(Debug, Clone, Copy)]
pub enum RepoFilter {
    /// Include all repositories (curated and custom).
    All,
    /// Include only curated repositories.
    Curated,
    /// Include only custom repositories.
    Custom,
}

/// Fetch repositories based on filter and configuration.
///
/// Merges curated and custom repositories based on the filter and config settings.
/// Deduplicates by full repository name.
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
/// Returns an error if configuration cannot be loaded or repositories cannot be fetched.
pub async fn fetch_all(filter: RepoFilter) -> crate::Result<Vec<CuratedRepo>> {
    let config = load_config()?;
    let mut repos = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Add curated repos if enabled and filter allows
    match filter {
        RepoFilter::All | RepoFilter::Curated => {
            if config.repos.curated {
                let curated = fetch().await?;
                for repo in curated {
                    if seen.insert(repo.full_name()) {
                        repos.push(repo);
                    }
                }
            }
        }
        RepoFilter::Custom => {}
    }

    // Add custom repos if filter allows
    match filter {
        RepoFilter::All | RepoFilter::Custom => {
            let custom = custom::read_custom_repos()?;
            for repo in custom {
                if seen.insert(repo.full_name()) {
                    repos.push(repo);
                }
            }
        }
        RepoFilter::Curated => {}
    }

    debug!(
        "Fetched {} repositories with filter {:?}",
        repos.len(),
        filter
    );
    Ok(repos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_name_format() {
        let repo = CuratedRepo {
            owner: "owner".to_string(),
            name: "repo".to_string(),
            language: "Rust".to_string(),
            description: "Test repository".to_string(),
        };
        assert_eq!(repo.full_name(), "owner/repo");
    }

    #[test]
    fn embedded_defaults_returns_non_empty() {
        let repos = embedded_defaults();
        assert!(
            !repos.is_empty(),
            "embedded defaults should contain repositories"
        );
    }
}
