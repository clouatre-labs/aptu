//! Configuration management for the Aptu CLI.
//!
// Allow dead_code - module will be used in PR 6 (CLI scaffolding)
#![allow(dead_code)]
//!
//! Provides layered configuration from files and environment variables.
//! Uses XDG-compliant paths via the `dirs` crate.
//!
//! # Configuration Sources (in priority order)
//!
//! 1. Environment variables (prefix: `APTU_`)
//! 2. Config file: `~/.config/aptu/config.toml` (or platform equivalent)
//! 3. Built-in defaults
//!
//! # Examples
//!
//! ```bash
//! # Override AI model via environment variable
//! APTU_AI__MODEL=mistral-small cargo run
//! ```

use std::path::PathBuf;

use config::{Config, Environment, File};
use serde::Deserialize;

use crate::error::AptuError;

/// Application configuration.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// User preferences.
    pub user: UserConfig,
    /// AI provider settings.
    pub ai: AiConfig,
    /// GitHub API settings.
    pub github: GitHubConfig,
    /// UI preferences.
    pub ui: UiConfig,
    /// Cache settings.
    pub cache: CacheConfig,
}

/// User preferences.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct UserConfig {
    /// Default repository to use (skip repo selection).
    pub default_repo: Option<String>,
}

/// AI provider settings.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// AI provider: "openrouter" or "ollama".
    pub provider: String,
    /// Model identifier.
    pub model: String,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Allow paid models (default: false for cost control).
    pub allow_paid_models: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "openrouter".to_string(),
            model: "mistralai/devstral-2512:free".to_string(),
            timeout_seconds: 30,
            allow_paid_models: false,
        }
    }
}

/// GitHub API settings.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GitHubConfig {
    /// API request timeout in seconds.
    pub api_timeout_seconds: u64,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            api_timeout_seconds: 10,
        }
    }
}

/// UI preferences.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Enable colored output.
    pub color: bool,
    /// Show progress bars.
    pub progress_bars: bool,
    /// Always confirm before posting comments.
    pub confirm_before_post: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            color: true,
            progress_bars: true,
            confirm_before_post: true,
        }
    }
}

/// Cache settings.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Issue cache TTL in minutes.
    pub issue_ttl_minutes: u64,
    /// Repository metadata cache TTL in hours.
    pub repo_ttl_hours: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            issue_ttl_minutes: 60,
            repo_ttl_hours: 24,
        }
    }
}

/// Returns the Aptu configuration directory.
///
/// - Linux: `~/.config/aptu`
/// - macOS: `~/Library/Application Support/aptu`
/// - Windows: `C:\Users\<User>\AppData\Roaming\aptu`
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("Could not determine config directory - is HOME set?")
        .join("aptu")
}

/// Returns the Aptu data directory.
///
/// - Linux: `~/.local/share/aptu`
/// - macOS: `~/Library/Application Support/aptu`
/// - Windows: `C:\Users\<User>\AppData\Local\aptu`
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .expect("Could not determine data directory - is HOME set?")
        .join("aptu")
}

/// Returns the path to the configuration file.
pub fn config_file_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Load application configuration.
///
/// Loads from config file (if exists) and environment variables.
/// Environment variables use the prefix `APTU_` and double underscore
/// for nested keys (e.g., `APTU_AI__MODEL`).
///
/// # Errors
///
/// Returns `AptuError::Config` if the config file exists but is invalid.
pub fn load_config() -> Result<AppConfig, AptuError> {
    let config_path = config_file_path();

    let config = Config::builder()
        // Load from config file (optional - may not exist)
        .add_source(File::with_name(config_path.to_string_lossy().as_ref()).required(false))
        // Override with environment variables
        .add_source(
            Environment::with_prefix("APTU")
                .separator("__")
                .try_parsing(true),
        )
        .build()?;

    let app_config: AppConfig = config.try_deserialize()?;

    Ok(app_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_defaults() {
        // Without any config file or env vars, should return defaults
        let config = load_config().expect("should load with defaults");

        assert_eq!(config.ai.provider, "openrouter");
        assert_eq!(config.ai.model, "mistralai/devstral-2512:free");
        assert_eq!(config.ai.timeout_seconds, 30);
        assert_eq!(config.github.api_timeout_seconds, 10);
        assert!(config.ui.color);
        assert!(config.ui.confirm_before_post);
        assert_eq!(config.cache.issue_ttl_minutes, 60);
    }

    #[test]
    fn test_config_dir_exists() {
        let dir = config_dir();
        assert!(dir.ends_with("aptu"));
    }

    #[test]
    fn test_data_dir_exists() {
        let dir = data_dir();
        assert!(dir.ends_with("aptu"));
    }

    #[test]
    fn test_config_file_path() {
        let path = config_file_path();
        assert!(path.ends_with("config.toml"));
    }
}
