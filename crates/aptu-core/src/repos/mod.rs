// SPDX-License-Identifier: Apache-2.0

//! Curated repository list for Aptu.
//!
//! Repositories are fetched from a remote JSON file with TTL-based caching.
//! The list contains repositories known to be:
//! - Active (commits in last 30 days)
//! - Welcoming (good first issue labels exist)
//! - Responsive (maintainers reply within 1 week)

use chrono::Duration;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::cache::{self, CacheEntry};
use crate::config::load_config;

/// Embedded curated repositories as fallback when network fetch fails.
const EMBEDDED_REPOS: &str = include_str!("../../../../data/curated-repos.json");

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
    let ttl = Duration::hours(config.cache.repo_ttl_hours.try_into().unwrap_or(24));

    // Try cache first
    let cache_key = "curated_repos.json";
    if let Ok(Some(entry)) = cache::read_cache::<Vec<CuratedRepo>>(cache_key)
        && entry.is_valid(ttl)
    {
        debug!("Using cached curated repositories");
        return Ok(entry.data);
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
    let entry = CacheEntry::new(repos.clone());
    let _ = cache::write_cache(cache_key, &entry);
    debug!("Fetched and cached {} curated repositories", repos.len());

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
