// SPDX-License-Identifier: Apache-2.0

//! Configuration management for the Aptu CLI.
//!
//! Provides layered configuration from files and environment variables.
//! Uses XDG-compliant paths via the `dirs` crate.
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
use serde::{Deserialize, Serialize};

use crate::error::AptuError;

/// Default `OpenRouter` model identifier.
pub const DEFAULT_OPENROUTER_MODEL: &str = "mistralai/mistral-small-2603";
/// Default `Gemini` model identifier.
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3.1-flash-lite-preview";

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
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
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
    /// PR review prompt settings.
    #[serde(default)]
    pub review: ReviewConfig,
}

/// User preferences.
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct UserConfig {
    /// Default repository to use (skip repo selection).
    pub default_repo: Option<String>,
}

/// Task-specific AI model override.
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct TaskOverride {
    /// Optional provider override for this task.
    pub provider: Option<String>,
    /// Optional model override for this task.
    pub model: Option<String>,
}

/// Task-specific AI configuration.
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct TasksConfig {
    /// Triage task configuration.
    pub triage: Option<TaskOverride>,
    /// Review task configuration.
    pub review: Option<TaskOverride>,
    /// Create task configuration.
    pub create: Option<TaskOverride>,
}

/// Single entry in the fallback provider chain.
#[derive(Debug, Clone, Serialize)]
pub struct FallbackEntry {
    /// Provider name (e.g., "openrouter", "anthropic", "gemini").
    pub provider: String,
    /// Optional model override for this specific provider.
    pub model: Option<String>,
}

impl<'de> Deserialize<'de> for FallbackEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum EntryVariant {
            String(String),
            Struct {
                provider: String,
                model: Option<String>,
            },
        }

        match EntryVariant::deserialize(deserializer)? {
            EntryVariant::String(provider) => Ok(FallbackEntry {
                provider,
                model: None,
            }),
            EntryVariant::Struct { provider, model } => Ok(FallbackEntry { provider, model }),
        }
    }
}

/// Fallback provider chain configuration.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct FallbackConfig {
    /// Chain of fallback entries to try in order when primary fails.
    pub chain: Vec<FallbackEntry>,
}

/// Default value for `retry_max_attempts`.
fn default_retry_max_attempts() -> u32 {
    3
}

/// AI provider settings.
#[derive(Debug, Deserialize, Serialize, Clone)]
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
    /// Maximum retry attempts for rate-limited requests (default: 3).
    #[serde(default = "default_retry_max_attempts")]
    pub retry_max_attempts: u32,
    /// Task-specific model overrides.
    pub tasks: Option<TasksConfig>,
    /// Fallback provider chain for resilience.
    pub fallback: Option<FallbackConfig>,
    /// Custom guidance to override or extend default best practices.
    ///
    /// Allows users to provide project-specific tooling recommendations
    /// that will be appended to the default best practices context.
    /// Useful for enforcing project-specific choices (e.g., poetry instead of uv).
    pub custom_guidance: Option<String>,
    /// Enable pre-flight model validation with fuzzy matching (default: true).
    ///
    /// When enabled, validates that the configured model ID exists in the
    /// cached model registry before creating an AI client. Provides helpful
    /// suggestions if an invalid model ID is detected.
    pub validation_enabled: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "openrouter".to_string(),
            model: DEFAULT_OPENROUTER_MODEL.to_string(),
            timeout_seconds: 30,
            allow_paid_models: true,
            max_tokens: 4096,
            temperature: 0.3,
            circuit_breaker_threshold: 3,
            circuit_breaker_reset_seconds: 60,
            retry_max_attempts: default_retry_max_attempts(),
            tasks: None,
            fallback: None,
            custom_guidance: None,
            validation_enabled: true,
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
#[derive(Debug, Deserialize, Serialize, Clone)]
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
#[derive(Debug, Deserialize, Serialize, Clone)]
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
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct CacheConfig {
    /// Issue cache TTL in minutes.
    pub issue_ttl_minutes: i64,
    /// Repository metadata cache TTL in hours.
    pub repo_ttl_hours: i64,
    /// URL to fetch curated repositories from.
    pub curated_repos_url: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            issue_ttl_minutes: crate::cache::DEFAULT_ISSUE_TTL_MINS,
            repo_ttl_hours: crate::cache::DEFAULT_REPO_TTL_HOURS,
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
}

impl Default for ReposConfig {
    fn default() -> Self {
        Self { curated: true }
    }
}

/// PR review prompt configuration.
///
/// Controls prompt token budgets and GitHub API constraints for PR reviews:
///
/// - `max_prompt_chars`: 120,000 chars is a conservative budget below common LLM context
///   window limits (e.g., 128k token models), accounting for system prompt and response overhead.
/// - `max_full_content_files`: 10 files caps GitHub Contents API calls per review to limit
///   latency and rate limit usage.
/// - `max_chars_per_file`: 4,000 chars per file keeps individual file snippets readable
///   without dominating the prompt budget.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ReviewConfig {
    /// Maximum total prompt character budget (default: `120_000`).
    pub max_prompt_chars: usize,
    /// Maximum number of files to fetch full content for (default: 10).
    pub max_full_content_files: usize,
    /// Maximum characters per file's full content (default: `4_000`).
    pub max_chars_per_file: usize,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            max_prompt_chars: 120_000, // Conservative budget for LLM context windows with overhead
            max_full_content_files: 10, // Cap GitHub Contents API calls to limit latency and rate limits
            max_chars_per_file: 4_000, // Keep individual file snippets readable without overwhelming prompt
        }
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

/// Returns the Aptu prompts configuration directory.
///
/// Prompt override files are loaded from this directory at runtime.
/// Place a `<name>.md` file here to override the compiled-in prompt.
///
/// Respects the `XDG_CONFIG_HOME` environment variable if set,
/// otherwise defaults to `~/.config/aptu/prompts`.
#[must_use]
pub fn prompts_dir() -> PathBuf {
    config_dir().join("prompts")
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
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        )
        .build()?;

    let app_config: AppConfig = config.try_deserialize()?;

    Ok(app_config)
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_load_config_defaults() {
        // Without any config file or env vars, should return defaults.
        // Point XDG_CONFIG_HOME to a guaranteed-empty temp dir so the real
        // user config (~/.config/aptu/config.toml) is not loaded.
        let tmp_dir = std::env::temp_dir().join("aptu_test_defaults_no_config");
        std::fs::create_dir_all(&tmp_dir).expect("create tmp dir");
        // SAFETY: single-threaded test process; no concurrent env reads.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &tmp_dir);
        }
        let config = load_config().expect("should load with defaults");
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        assert_eq!(config.ai.provider, "openrouter");
        assert_eq!(config.ai.model, DEFAULT_OPENROUTER_MODEL);
        assert_eq!(config.ai.timeout_seconds, 30);
        assert_eq!(config.ai.max_tokens, 4096);
        assert_eq!(config.ai.allow_paid_models, true);
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(config.ai.temperature, 0.3);
        }
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
model = "gemini-3.1-flash-lite-preview"

[ai.tasks.triage]
model = "gemini-3.1-flash-lite-preview"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert_eq!(app_config.ai.provider, "gemini");
        assert_eq!(app_config.ai.model, DEFAULT_GEMINI_MODEL);
        assert!(app_config.ai.tasks.is_some());

        let tasks = app_config.ai.tasks.unwrap();
        assert!(tasks.triage.is_some());
        assert!(tasks.review.is_none());
        assert!(tasks.create.is_none());

        let triage = tasks.triage.unwrap();
        assert_eq!(triage.provider, None);
        assert_eq!(triage.model, Some(DEFAULT_GEMINI_MODEL.to_string()));
    }

    #[test]
    fn test_config_with_multiple_task_overrides() {
        // Test that config with multiple task overrides parses correctly
        let config_str = r#"
[ai]
provider = "openrouter"
model = "mistralai/mistral-small-2603"

[ai.tasks.triage]
model = "mistralai/mistral-small-2603"

[ai.tasks.review]
provider = "openrouter"
model = "anthropic/claude-haiku-4.5"

[ai.tasks.create]
model = "anthropic/claude-sonnet-4.6"
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
        assert_eq!(triage.model, Some(DEFAULT_OPENROUTER_MODEL.to_string()));

        // Review: both provider and model override
        let review = tasks.review.expect("review should exist");
        assert_eq!(review.provider, Some("openrouter".to_string()));
        assert_eq!(review.model, Some("anthropic/claude-haiku-4.5".to_string()));

        // Create: only model override
        let create = tasks.create.expect("create should exist");
        assert_eq!(create.provider, None);
        assert_eq!(
            create.model,
            Some("anthropic/claude-sonnet-4.6".to_string())
        );
    }

    #[test]
    fn test_config_with_partial_task_overrides() {
        // Test that partial task configs (only provider or only model) parse correctly
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

[ai.tasks.triage]
provider = "gemini"

[ai.tasks.review]
model = "gemini-3.1-flash-lite-preview"
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
        assert_eq!(review.model, Some(DEFAULT_GEMINI_MODEL.to_string()));
    }

    #[test]
    fn test_config_without_tasks_section() {
        // Test that config without explicit tasks section uses defaults
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert_eq!(app_config.ai.provider, "gemini");
        assert_eq!(app_config.ai.model, DEFAULT_GEMINI_MODEL);
        // When no tasks section is provided, defaults are used (tasks: None)
        assert!(app_config.ai.tasks.is_none());
    }

    #[test]
    fn test_resolve_for_task_with_defaults() {
        // Test that resolve_for_task returns correct defaults (all tasks use openrouter)
        let ai_config = AiConfig::default();

        // All tasks use global defaults (openrouter/mistralai/mistral-small-2603)
        let (provider, model) = ai_config.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, DEFAULT_OPENROUTER_MODEL);
        assert_eq!(ai_config.allow_paid_models, true);

        let (provider, model) = ai_config.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, DEFAULT_OPENROUTER_MODEL);
        assert_eq!(ai_config.allow_paid_models, true);

        let (provider, model) = ai_config.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "mistralai/mistral-small-2603");
        assert_eq!(ai_config.allow_paid_models, true);
    }

    #[test]
    fn test_resolve_for_task_with_triage_override() {
        // Test that resolve_for_task returns triage override when present
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

[ai.tasks.triage]
model = "gemini-3.1-flash-lite-preview"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Triage should use override
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "gemini");
        assert_eq!(model, DEFAULT_GEMINI_MODEL);

        // Review and Create should use defaults
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "gemini");
        assert_eq!(model, DEFAULT_GEMINI_MODEL);

        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, DEFAULT_GEMINI_MODEL);
    }

    #[test]
    fn test_resolve_for_task_with_provider_override() {
        // Test that resolve_for_task returns provider override when present
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

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
        assert_eq!(model, DEFAULT_GEMINI_MODEL);

        // Triage and Create should use defaults
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "gemini");
        assert_eq!(model, DEFAULT_GEMINI_MODEL);

        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, DEFAULT_GEMINI_MODEL);
    }

    #[test]
    fn test_resolve_for_task_with_full_overrides() {
        // Test that resolve_for_task returns both provider and model overrides
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

[ai.tasks.triage]
provider = "openrouter"
model = "mistralai/mistral-small-2603"

[ai.tasks.review]
provider = "openrouter"
model = "anthropic/claude-haiku-4.5"

[ai.tasks.create]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Triage
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Triage);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, DEFAULT_OPENROUTER_MODEL);

        // Review
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "anthropic/claude-haiku-4.5");

        // Create
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, DEFAULT_GEMINI_MODEL);
    }

    #[test]
    fn test_resolve_for_task_partial_overrides() {
        // Test that resolve_for_task handles partial overrides correctly
        let config_str = r#"
[ai]
provider = "openrouter"
model = "mistralai/mistral-small-2603"

[ai.tasks.triage]
model = "mistralai/mistral-small-2603"

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
        assert_eq!(model, DEFAULT_OPENROUTER_MODEL);

        // Review: provider override, model from default
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, DEFAULT_OPENROUTER_MODEL);

        // Create: empty override, both from default
        let (provider, model) = app_config.ai.resolve_for_task(TaskType::Create);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, DEFAULT_OPENROUTER_MODEL);
    }

    #[test]
    fn test_fallback_config_toml_parsing() {
        // Test that FallbackConfig deserializes from TOML correctly
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

[ai.fallback]
chain = ["openrouter", "anthropic"]
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert_eq!(app_config.ai.provider, "gemini");
        assert_eq!(app_config.ai.model, "gemini-3.1-flash-lite-preview");
        assert!(app_config.ai.fallback.is_some());

        let fallback = app_config.ai.fallback.unwrap();
        assert_eq!(fallback.chain.len(), 2);
        assert_eq!(fallback.chain[0].provider, "openrouter");
        assert_eq!(fallback.chain[1].provider, "anthropic");
    }

    #[test]
    fn test_fallback_config_empty_chain() {
        // Test that FallbackConfig with empty chain parses correctly
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

[ai.fallback]
chain = []
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert!(app_config.ai.fallback.is_some());
        let fallback = app_config.ai.fallback.unwrap();
        assert_eq!(fallback.chain.len(), 0);
    }

    #[test]
    fn test_fallback_config_single_provider() {
        // Test that FallbackConfig with single provider parses correctly
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

[ai.fallback]
chain = ["openrouter"]
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert!(app_config.ai.fallback.is_some());
        let fallback = app_config.ai.fallback.unwrap();
        assert_eq!(fallback.chain.len(), 1);
        assert_eq!(fallback.chain[0].provider, "openrouter");
    }

    #[test]
    fn test_fallback_config_without_fallback_section() {
        // Test that config without fallback section has None
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert!(app_config.ai.fallback.is_none());
    }

    #[test]
    fn test_fallback_config_default() {
        // Test that AiConfig::default() has fallback: None
        let ai_config = AiConfig::default();
        assert!(ai_config.fallback.is_none());
    }

    #[test]
    #[serial]
    fn test_load_config_env_var_override() {
        // Test that APTU_AI__MODEL and APTU_AI__PROVIDER env vars override defaults.
        let tmp_dir = std::env::temp_dir().join("aptu_test_env_override");
        std::fs::create_dir_all(&tmp_dir).expect("create tmp dir");
        // SAFETY: single-threaded test process; no concurrent env reads.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &tmp_dir);
            std::env::set_var("APTU_AI__MODEL", "test-model-override");
            std::env::set_var("APTU_AI__PROVIDER", "openrouter");
        }
        let config = load_config().expect("should load with env overrides");
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("APTU_AI__MODEL");
            std::env::remove_var("APTU_AI__PROVIDER");
        }

        assert_eq!(config.ai.model, "test-model-override");
        assert_eq!(config.ai.provider, "openrouter");
    }
}
