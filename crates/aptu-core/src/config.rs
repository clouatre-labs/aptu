// SPDX-License-Identifier: Apache-2.0

//! Configuration management for the Aptu CLI.
//!
//! Provides layered configuration from files and environment variables.
//! Uses XDG-compliant paths with environment variable support.
//!
//! # Configuration Sources (in priority order)
//!
//! 1. Environment variables (prefix: `APTU_`)
//! 2. Config file: `~/.config/aptu/config.toml`
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

/// Task type for model selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// Issue triage task.
    Triage,
    /// Pull request review task.
    Review,
    /// Label creation task.
    Create,
}

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
    /// Repository settings.
    pub repos: ReposConfig,
}

/// User preferences.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct UserConfig {
    /// Default repository to use (skip repo selection).
    pub default_repo: Option<String>,
}

/// Task-specific AI model override.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct TaskOverride {
    /// Optional provider override for this task.
    pub provider: Option<String>,
    /// Optional model override for this task.
    pub model: Option<String>,
}

/// Task-specific AI configuration.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct TasksConfig {
    /// Triage task configuration.
    pub triage: Option<TaskOverride>,
    /// Review task configuration.
    pub review: Option<TaskOverride>,
    /// Create task configuration.
    pub create: Option<TaskOverride>,
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
    /// Maximum tokens for API responses.
    pub max_tokens: u32,
    /// Temperature for API requests (0.0-1.0).
    pub temperature: f32,
    /// Circuit breaker failure threshold before opening (default: 3).
    pub circuit_breaker_threshold: u32,
    /// Circuit breaker reset timeout in seconds (default: 60).
    pub circuit_breaker_reset_seconds: u64,
    /// Task-specific model overrides.
    pub tasks: Option<TasksConfig>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "gemini".to_string(),
            model: "gemini-3-flash-preview".to_string(),
            timeout_seconds: 30,
            allow_paid_models: false,
            max_tokens: 4096,
            temperature: 0.3,
            circuit_breaker_threshold: 3,
            circuit_breaker_reset_seconds: 60,
            tasks: None,
        }
    }
}

impl AiConfig {
    /// Resolve provider and model for a specific task type.
    ///
    /// Returns a tuple of (provider, model) by checking task-specific overrides first,
    /// then falling back to the default provider and model.
    ///
    /// # Arguments
    ///
    /// * `task` - The task type to resolve configuration for
    ///
    /// # Returns
    ///
    /// A tuple of (`provider_name`, `model_name`) strings
    #[must_use]
    pub fn resolve_for_task(&self, task: TaskType) -> (String, String) {
        let task_override = match task {
            TaskType::Triage => self.tasks.as_ref().and_then(|t| t.triage.as_ref()),
            TaskType::Review => self.tasks.as_ref().and_then(|t| t.review.as_ref()),
            TaskType::Create => self.tasks.as_ref().and_then(|t| t.create.as_ref()),
        };

        let provider = task_override
            .and_then(|o| o.provider.clone())
            .unwrap_or_else(|| self.provider.clone());

        let model = task_override
            .and_then(|o| o.model.clone())
            .unwrap_or_else(|| self.model.clone());

        (provider, model)
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
    /// URL to fetch curated repositories from.
    pub curated_repos_url: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            issue_ttl_minutes: 60,
            repo_ttl_hours: 24,
            curated_repos_url:
                "https://raw.githubusercontent.com/clouatre-labs/aptu/main/data/curated-repos.json"
                    .to_string(),
        }
    }
}

/// Repository settings.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ReposConfig {
    /// Include curated repositories (default: true).
    pub curated: bool,
}

impl Default for ReposConfig {
    fn default() -> Self {
        Self { curated: true }
    }
}

/// Returns the Aptu configuration directory.
///
/// Respects the `XDG_CONFIG_HOME` environment variable if set,
/// otherwise defaults to `~/.config/aptu`.
#[must_use]
pub fn config_dir() -> PathBuf {
    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME")
        && !xdg_config.is_empty()
    {
        return PathBuf::from(xdg_config).join("aptu");
    }
    dirs::home_dir()
        .expect("Could not determine home directory - is HOME set?")
        .join(".config")
        .join("aptu")
}

/// Returns the Aptu data directory.
///
/// Respects the `XDG_DATA_HOME` environment variable if set,
/// otherwise defaults to `~/.local/share/aptu`.
#[must_use]
pub fn data_dir() -> PathBuf {
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME")
        && !xdg_data.is_empty()
    {
        return PathBuf::from(xdg_data).join("aptu");
    }
    dirs::home_dir()
        .expect("Could not determine home directory - is HOME set?")
        .join(".local")
        .join("share")
        .join("aptu")
}

/// Returns the path to the configuration file.
#[must_use]
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

        assert_eq!(config.ai.provider, "gemini");
        assert_eq!(config.ai.model, "gemini-3-flash-preview");
        assert_eq!(config.ai.timeout_seconds, 30);
        assert_eq!(config.ai.max_tokens, 4096);
        assert_eq!(config.ai.temperature, 0.3);
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

    #[test]
    fn test_config_with_task_triage_override() {
        // Test that config with [ai.tasks.triage] parses correctly
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3-flash-preview"

[ai.tasks.triage]
model = "gemini-2.5-flash-lite-preview-09-2025"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert_eq!(app_config.ai.provider, "gemini");
        assert_eq!(app_config.ai.model, "gemini-3-flash-preview");
        assert!(app_config.ai.tasks.is_some());

        let tasks = app_config.ai.tasks.unwrap();
        assert!(tasks.triage.is_some());
        assert!(tasks.review.is_none());
        assert!(tasks.create.is_none());

        let triage = tasks.triage.unwrap();
        assert_eq!(triage.provider, None);
        assert_eq!(
            triage.model,
            Some("gemini-2.5-flash-lite-preview-09-2025".to_string())
        );
    }

    #[test]
    fn test_config_with_multiple_task_overrides() {
        // Test that config with multiple task overrides parses correctly
        let config_str = r#"
[ai]
provider = "openrouter"
model = "mistralai/devstral-2512:free"

[ai.tasks.triage]
model = "mistralai/devstral-2512:free"

[ai.tasks.review]
provider = "openrouter"
model = "anthropic/claude-haiku-4.5"

[ai.tasks.create]
model = "anthropic/claude-sonnet-4.5"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        let tasks = app_config.ai.tasks.expect("tasks should exist");

        // Triage: only model override
        let triage = tasks.triage.expect("triage should exist");
        assert_eq!(triage.provider, None);
        assert_eq!(
            triage.model,
            Some("mistralai/devstral-2512:free".to_string())
        );

        // Review: both provider and model override
        let review = tasks.review.expect("review should exist");
        assert_eq!(review.provider, Some("openrouter".to_string()));
        assert_eq!(review.model, Some("anthropic/claude-haiku-4.5".to_string()));

        // Create: only model override
        let create = tasks.create.expect("create should exist");
        assert_eq!(create.provider, None);
        assert_eq!(
            create.model,
            Some("anthropic/claude-sonnet-4.5".to_string())
        );
    }

    #[test]
    fn test_config_with_partial_task_overrides() {
        // Test that partial task configs (only provider or only model) parse correctly
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3-flash-preview"

[ai.tasks.triage]
provider = "gemini"

[ai.tasks.review]
model = "gemini-3-pro-preview"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        let tasks = app_config.ai.tasks.expect("tasks should exist");

        // Triage: only provider
        let triage = tasks.triage.expect("triage should exist");
        assert_eq!(triage.provider, Some("gemini".to_string()));
        assert_eq!(triage.model, None);

        // Review: only model
        let review = tasks.review.expect("review should exist");
        assert_eq!(review.provider, None);
        assert_eq!(review.model, Some("gemini-3-pro-preview".to_string()));
    }

    #[test]
    fn test_config_without_tasks_section() {
        // Test that default config loads without tasks section
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3-flash-preview"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert_eq!(app_config.ai.provider, "gemini");
        assert_eq!(app_config.ai.model, "gemini-3-flash-preview");
        assert!(app_config.ai.tasks.is_none());
    }

    #[test]
    fn test_resolve_for_task_no_overrides() {
        // Test that resolve_for_task returns defaults when no task overrides exist
        let ai_config = AiConfig::default();

        let (provider, model) = ai_config.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-3-flash-preview");

        let (provider, model) = ai_config.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-3-flash-preview");

        let (provider, model) = ai_config.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-3-flash-preview");
    }

    #[test]
    fn test_resolve_for_task_with_triage_override() {
        // Test that resolve_for_task returns triage override when present
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3-flash-preview"

[ai.tasks.triage]
model = "gemini-2.5-flash-lite-preview-09-2025"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Triage should use override
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-2.5-flash-lite-preview-09-2025");

        // Review and Create should use defaults
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-3-flash-preview");

        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-3-flash-preview");
    }

    #[test]
    fn test_resolve_for_task_with_provider_override() {
        // Test that resolve_for_task returns provider override when present
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3-flash-preview"

[ai.tasks.review]
provider = "openrouter"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Review should use provider override but default model
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "gemini-3-flash-preview");

        // Triage and Create should use defaults
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-3-flash-preview");

        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-3-flash-preview");
    }

    #[test]
    fn test_resolve_for_task_with_full_overrides() {
        // Test that resolve_for_task returns both provider and model overrides
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3-flash-preview"

[ai.tasks.triage]
provider = "openrouter"
model = "mistralai/devstral-2512:free"

[ai.tasks.review]
provider = "openrouter"
model = "anthropic/claude-haiku-4.5"

[ai.tasks.create]
provider = "gemini"
model = "gemini-3-pro-preview"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Triage
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "mistralai/devstral-2512:free");

        // Review
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "anthropic/claude-haiku-4.5");

        // Create
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gemini-3-pro-preview");
    }

    #[test]
    fn test_resolve_for_task_partial_overrides() {
        // Test that resolve_for_task handles partial overrides correctly
        let config_str = r#"
[ai]
provider = "openrouter"
model = "mistralai/devstral-2512:free"

[ai.tasks.triage]
model = "mistralai/devstral-2512:free"

[ai.tasks.review]
provider = "openrouter"

[ai.tasks.create]
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Triage: model override, provider from default
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "mistralai/devstral-2512:free");

        // Review: provider override, model from default
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "mistralai/devstral-2512:free");

        // Create: empty override, both from default
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "mistralai/devstral-2512:free");
    }

    #[test]
    fn test_config_dir_respects_xdg_config_home() {
        // Test that config_dir respects XDG_CONFIG_HOME when set
        let original = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", "/custom/config");
        }

        let dir = config_dir();
        assert_eq!(dir, PathBuf::from("/custom/config/aptu"));

        // Cleanup
        unsafe {
            match original {
                Some(val) => std::env::set_var("XDG_CONFIG_HOME", val),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }

    #[test]
    fn test_config_dir_ignores_empty_xdg_config_home() {
        // Test that config_dir ignores empty XDG_CONFIG_HOME
        let original = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", "");
        }

        let dir = config_dir();
        assert!(dir.ends_with("aptu"));

        // Cleanup
        unsafe {
            match original {
                Some(val) => std::env::set_var("XDG_CONFIG_HOME", val),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }

    #[test]
    fn test_data_dir_respects_xdg_data_home() {
        // Test that data_dir respects XDG_DATA_HOME when set
        let original = std::env::var("XDG_DATA_HOME").ok();
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "/custom/data");
        }

        let dir = data_dir();
        assert_eq!(dir, PathBuf::from("/custom/data/aptu"));

        // Cleanup
        unsafe {
            match original {
                Some(val) => std::env::set_var("XDG_DATA_HOME", val),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
    }

    #[test]
    fn test_data_dir_ignores_empty_xdg_data_home() {
        // Test that data_dir ignores empty XDG_DATA_HOME
        let original = std::env::var("XDG_DATA_HOME").ok();
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "");
        }

        let dir = data_dir();
        assert!(dir.ends_with("aptu"));

        // Cleanup
        unsafe {
            match original {
                Some(val) => std::env::set_var("XDG_DATA_HOME", val),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
    }
}
