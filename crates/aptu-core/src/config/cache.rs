// SPDX-License-Identifier: Apache-2.0

//! Cache configuration.

use serde::{Deserialize, Serialize};

/// Cache settings.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct CacheConfig {
    /// URL to fetch curated repositories from.
    pub curated_repos_url: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            curated_repos_url:
                "https://raw.githubusercontent.com/clouatre-labs/aptu/main/data/curated-repos.json"
                    .to_string(),
        }
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

#[cfg(test)]
mod tests {
    use super::CacheConfig;

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert!(config.curated_repos_url.starts_with("https://"));
    }
}
