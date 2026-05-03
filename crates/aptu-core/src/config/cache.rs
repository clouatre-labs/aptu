// SPDX-License-Identifier: Apache-2.0

//! Cache configuration.

use serde::{Deserialize, Serialize};

/// Cache settings.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct CacheConfig {
    /// Issue cache TTL in minutes.
    pub issue_ttl_minutes: i64,
    /// Repository metadata cache TTL in hours.
    pub repo_ttl_hours: i64,
    /// URL to fetch curated repositories from.
    pub curated_repos_url: String,
    /// File eviction TTL in days (default: 7).
    pub file_eviction_days: i64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            issue_ttl_minutes: crate::cache::DEFAULT_ISSUE_TTL_MINS,
            repo_ttl_hours: crate::cache::DEFAULT_REPO_TTL_HOURS,
            curated_repos_url:
                "https://raw.githubusercontent.com/clouatre-labs/aptu/main/data/curated-repos.json"
                    .to_string(),
            file_eviction_days: 7,
        }
    }
}

impl CacheConfig {
    /// Validates cache configuration.
    ///
    /// Ensures `file_eviction_days` is positive (> 0).
    ///
    /// # Errors
    ///
    /// Returns an error if `file_eviction_days <= 0`.
    pub fn validate(&self) -> Result<(), String> {
        if self.file_eviction_days <= 0 {
            return Err(format!(
                "file_eviction_days must be > 0, got {}",
                self.file_eviction_days
            ));
        }
        Ok(())
    }
}

/// Repository settings.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ReposConfig {
    /// Include curated repositories (default: true).
    pub curated: bool,
    /// DCO sign-off on commits (default: false).
    #[serde(default)]
    pub dco_signoff: bool,
}

impl Default for ReposConfig {
    fn default() -> Self {
        Self {
            curated: true,
            dco_signoff: false,
        }
    }
}
