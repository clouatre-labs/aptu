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
///
/// By default, includes sensible ignore paths for common test and vendor directories.
/// Use `SecurityConfig::empty()` for a configuration with no ignore rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Pattern IDs to ignore (e.g., `["hardcoded-secret", "sql-injection"]`).
    #[serde(default)]
    pub ignore_patterns: Vec<String>,

    /// File path prefixes to ignore (e.g., `["test/", "vendor/"]`).
    #[serde(default)]
    pub ignore_paths: Vec<String>,
}

impl Default for SecurityConfig {
    /// Returns configuration with sensible default ignore paths.
    ///
    /// Includes common test and vendor directories that typically contain
    /// test fixtures or third-party code that should not be scanned.
    fn default() -> Self {
        Self {
            ignore_patterns: vec![],
            ignore_paths: vec![
                "tests/".to_string(),
                "test/".to_string(),
                "benches/".to_string(),
                "fixtures/".to_string(),
                "vendor/".to_string(),
            ],
        }
    }
}

impl SecurityConfig {
    /// Create configuration with sensible default ignore paths.
    ///
    /// This is an alias for `Default::default()`.
    #[must_use]
    #[deprecated(since = "0.6.0", note = "Use `SecurityConfig::default()` instead")]
    pub fn with_defaults() -> Self {
        Self::default()
    }

    /// Create an empty configuration with no ignore rules.
    ///
    /// Use this when you want to scan all files without any filtering.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            ignore_patterns: vec![],
            ignore_paths: vec![],
        }
    }

    /// Check if a file path should be ignored based on configuration.
    ///
    /// This is a fast check that can be used before scanning to avoid
    /// running expensive regex patterns on files in ignored directories.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The file path to check
    ///
    /// # Returns
    ///
    /// `true` if the path should be ignored, `false` otherwise.
    #[must_use]
    pub fn should_ignore_path(&self, file_path: &str) -> bool {
        self.ignore_paths
            .iter()
            .any(|prefix| file_path.starts_with(prefix))
    }

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
    fn test_security_config_default_has_sensible_paths() {
        let config = SecurityConfig::default();
        assert!(config.ignore_patterns.is_empty());
        assert_eq!(config.ignore_paths.len(), 5);
        assert!(config.ignore_paths.contains(&"tests/".to_string()));
        assert!(config.ignore_paths.contains(&"test/".to_string()));
        assert!(config.ignore_paths.contains(&"benches/".to_string()));
        assert!(config.ignore_paths.contains(&"fixtures/".to_string()));
        assert!(config.ignore_paths.contains(&"vendor/".to_string()));
    }

    #[test]
    fn test_empty_config() {
        let config = SecurityConfig::empty();
        assert!(config.ignore_patterns.is_empty());
        assert!(config.ignore_paths.is_empty());
    }

    #[test]
    #[allow(deprecated)]
    fn test_with_defaults_deprecated() {
        // with_defaults is deprecated but should still work
        let config = SecurityConfig::with_defaults();
        assert!(config.ignore_patterns.is_empty());
        assert_eq!(config.ignore_paths.len(), 5);
    }

    #[test]
    fn test_should_ignore_path_method() {
        let config = SecurityConfig::default();

        // Should ignore test paths
        assert!(config.should_ignore_path("tests/unit/test.rs"));
        assert!(config.should_ignore_path("test/fixtures/data.rs"));
        assert!(config.should_ignore_path("vendor/lib.rs"));

        // Should not ignore src paths
        assert!(!config.should_ignore_path("src/main.rs"));
        assert!(!config.should_ignore_path("src/test.rs"));
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
    fn test_load_nonexistent_file_returns_defaults() {
        let path = PathBuf::from("/nonexistent/path/security.toml");
        let config = SecurityConfig::load_from_path(&path).expect("load default");
        // When file doesn't exist, should return sensible defaults
        assert!(config.ignore_patterns.is_empty());
        assert_eq!(config.ignore_paths.len(), 5);
    }

    #[test]
    fn test_config_path() {
        if let Some(path) = SecurityConfig::config_path() {
            assert!(path.ends_with("aptu/security.toml"));
        }
        // If None, test passes (config dir not available in environment)
    }
}
