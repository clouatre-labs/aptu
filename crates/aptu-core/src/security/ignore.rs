// SPDX-License-Identifier: Apache-2.0

//! Global ignore list for security findings.
//!
//! Allows users to configure patterns and paths to skip before LLM validation,
//! reducing API costs and noise from known false positives.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::Finding;

/// Security configuration for ignore rules.
///
/// Loaded from `~/.config/aptu/security.toml` with fallback to defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Pattern IDs to ignore (e.g., `["hardcoded-secret", "sql-injection"]`).
    #[serde(default)]
    pub ignore_patterns: Vec<String>,

    /// File path prefixes to ignore (e.g., `["test/", "vendor/"]`).
    #[serde(default)]
    pub ignore_paths: Vec<String>,
}

impl SecurityConfig {
    /// Load configuration from `~/.config/aptu/security.toml`.
    ///
    /// Returns default configuration if file doesn't exist or parse fails.
    ///
    /// # Returns
    ///
    /// Loaded configuration or default on error.
    #[must_use]
    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            match Self::load_from_path(&path) {
                Ok(config) => config,
                Err(e) => {
                    tracing::warn!("Failed to load security config: {:#}", e);
                    Self::default()
                }
            }
        } else {
            tracing::warn!("Config directory not available, using default security config");
            Self::default()
        }
    }

    /// Get the configuration file path.
    ///
    /// Returns `~/.config/aptu/security.toml` or `None` if config directory cannot be determined.
    #[must_use]
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("aptu").join("security.toml"))
    }

    /// Load configuration from a specific path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to configuration file
    ///
    /// # Returns
    ///
    /// Loaded configuration or error if file exists but is invalid.
    fn load_from_path(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }

    /// Check if a finding should be ignored based on configuration.
    ///
    /// A finding is ignored if:
    /// - Its pattern ID matches any entry in `ignore_patterns`
    /// - Its file path starts with any entry in `ignore_paths`
    ///
    /// # Arguments
    ///
    /// * `finding` - The finding to check
    ///
    /// # Returns
    ///
    /// `true` if the finding should be ignored, `false` otherwise.
    #[must_use]
    pub fn should_ignore(&self, finding: &Finding) -> bool {
        // Check pattern ID
        if self.ignore_patterns.contains(&finding.pattern_id) {
            return true;
        }

        // Check file path prefixes
        for prefix in &self.ignore_paths {
            if finding.file_path.starts_with(prefix) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{Confidence, Severity};

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.ignore_patterns.is_empty());
        assert!(config.ignore_paths.is_empty());
    }

    #[test]
    fn test_should_ignore_pattern() {
        let config = SecurityConfig {
            ignore_patterns: vec!["test-pattern".to_string(), "another-pattern".to_string()],
            ignore_paths: vec![],
        };

        let finding = Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test".to_string(),
            severity: Severity::Low,
            confidence: Confidence::Low,
            file_path: "src/main.rs".to_string(),
            line_number: 1,
            matched_text: "test".to_string(),
            cwe: None,
        };

        assert!(config.should_ignore(&finding));
    }

    #[test]
    fn test_should_ignore_path() {
        let config = SecurityConfig {
            ignore_patterns: vec![],
            ignore_paths: vec!["test/".to_string(), "vendor/".to_string()],
        };

        let finding = Finding {
            pattern_id: "pattern".to_string(),
            description: "Test".to_string(),
            severity: Severity::Low,
            confidence: Confidence::Low,
            file_path: "test/fixtures/data.rs".to_string(),
            line_number: 1,
            matched_text: "test".to_string(),
            cwe: None,
        };

        assert!(config.should_ignore(&finding));
    }

    #[test]
    fn test_should_not_ignore() {
        let config = SecurityConfig {
            ignore_patterns: vec!["other-pattern".to_string()],
            ignore_paths: vec!["vendor/".to_string()],
        };

        let finding = Finding {
            pattern_id: "real-pattern".to_string(),
            description: "Test".to_string(),
            severity: Severity::High,
            confidence: Confidence::High,
            file_path: "src/main.rs".to_string(),
            line_number: 42,
            matched_text: "code".to_string(),
            cwe: Some("CWE-123".to_string()),
        };

        assert!(!config.should_ignore(&finding));
    }

    #[test]
    fn test_should_ignore_path_prefix() {
        let config = SecurityConfig {
            ignore_patterns: vec![],
            ignore_paths: vec!["test/".to_string()],
        };

        // Should match prefix
        let finding1 = Finding {
            pattern_id: "pattern".to_string(),
            description: "Test".to_string(),
            severity: Severity::Low,
            confidence: Confidence::Low,
            file_path: "test/unit/test.rs".to_string(),
            line_number: 1,
            matched_text: "test".to_string(),
            cwe: None,
        };
        assert!(config.should_ignore(&finding1));

        // Should not match if not a prefix
        let finding2 = Finding {
            pattern_id: "pattern".to_string(),
            description: "Test".to_string(),
            severity: Severity::Low,
            confidence: Confidence::Low,
            file_path: "src/test.rs".to_string(),
            line_number: 1,
            matched_text: "test".to_string(),
            cwe: None,
        };
        assert!(!config.should_ignore(&finding2));
    }

    #[test]
    fn test_config_serialization() {
        let config = SecurityConfig {
            ignore_patterns: vec!["pattern1".to_string(), "pattern2".to_string()],
            ignore_paths: vec!["test/".to_string(), "vendor/".to_string()],
        };

        let toml = toml::to_string(&config).expect("serialize");
        let deserialized: SecurityConfig = toml::from_str(&toml).expect("deserialize");

        assert_eq!(config.ignore_patterns, deserialized.ignore_patterns);
        assert_eq!(config.ignore_paths, deserialized.ignore_paths);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/path/security.toml");
        let config = SecurityConfig::load_from_path(&path).expect("load default");
        assert!(config.ignore_patterns.is_empty());
        assert!(config.ignore_paths.is_empty());
    }

    #[test]
    fn test_config_path() {
        if let Some(path) = SecurityConfig::config_path() {
            assert!(path.ends_with("aptu/security.toml"));
        }
        // If None, test passes (config dir not available in environment)
    }
}
