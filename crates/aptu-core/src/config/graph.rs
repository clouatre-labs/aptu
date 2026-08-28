// SPDX-License-Identifier: Apache-2.0

//! Structural graph configuration for PR review context.

use serde::{Deserialize, Serialize};

/// Structural graph configuration.
///
/// Controls whether the petgraph-backed call graph is built for PR review
/// context, how long cached graphs remain valid, and the maximum blast-radius
/// subgraph size injected into the prompt:
///
/// - `enabled`: disabled by default; opt-in via config or CLI flag.
/// - `cache_ttl_hours`: 24 hours balances staleness against rebuild cost for
///   repositories with frequent commits.
/// - `max_nodes`: 50,000 nodes caps the blast-radius subgraph and cache size
///   for very large repositories.
/// - `max_depth`: 4 hops caps the blast-radius BFS traversal depth.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct GraphConfig {
    /// Whether structural graph context is enabled (default: `false`).
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Cache time-to-live in hours before a cached graph is rebuilt (default: `24`).
    #[serde(default = "default_cache_ttl_hours")]
    pub cache_ttl_hours: u64,
    /// Maximum number of nodes in the blast-radius subgraph (default: `50_000`).
    #[serde(default = "default_max_nodes")]
    pub max_nodes: usize,
    /// Maximum BFS hop depth from a modified node in the blast-radius subgraph (default: `4`).
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

fn default_enabled() -> bool {
    false
}

fn default_cache_ttl_hours() -> u64 {
    24
}

fn default_max_nodes() -> usize {
    50_000
}

fn default_max_depth() -> usize {
    4
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            cache_ttl_hours: default_cache_ttl_hours(),
            max_nodes: default_max_nodes(),
            max_depth: default_max_depth(),
        }
    }
}

impl GraphConfig {
    /// Validate internal consistency of graph configuration.
    ///
    /// Returns a list of warning strings for any misconfigured values.
    /// The caller should emit these warnings via `tracing::warn!` or similar.
    #[must_use]
    pub fn validate_consistency(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.enabled && self.max_nodes == 0 {
            warnings.push(
                "max_nodes is 0 while graph is enabled: blast-radius subgraph will always be empty"
                    .to_string(),
            );
        }

        if self.enabled && self.max_depth == 0 {
            warnings.push(
                "max_depth is 0 while graph is enabled: blast-radius subgraph will always be empty"
                    .to_string(),
            );
        }

        if self.enabled && self.cache_ttl_hours == 0 {
            warnings.push(
                "cache_ttl_hours is 0 while graph is enabled: cache is rebuilt on every review"
                    .to_string(),
            );
        }

        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_disabled() {
        let config = GraphConfig::default();
        assert!(!config.enabled, "graph should be disabled by default");
        assert_eq!(config.cache_ttl_hours, 24);
        assert_eq!(config.max_nodes, 50_000);
    }

    #[test]
    fn test_validate_consistency_ok() {
        let config = GraphConfig::default();
        let warnings = config.validate_consistency();
        assert!(
            warnings.is_empty(),
            "default config should produce no warnings: {warnings:?}"
        );
    }

    #[test]
    fn test_validate_consistency_zero_max_nodes_enabled() {
        let config = GraphConfig {
            enabled: true,
            max_nodes: 0,
            ..GraphConfig::default()
        };
        let warnings = config.validate_consistency();
        assert_eq!(warnings.len(), 1, "should produce exactly 1 warning");
        assert!(warnings[0].contains("max_nodes is 0"));
    }

    #[test]
    fn test_deserializes_from_toml_with_missing_fields() {
        let toml_str = "";
        let config: GraphConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.cache_ttl_hours, 24);
        assert_eq!(config.max_nodes, 50_000);
    }
}
