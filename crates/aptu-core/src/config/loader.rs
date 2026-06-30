// SPDX-License-Identifier: Apache-2.0

//! Configuration loading and path management.

use std::path::PathBuf;

use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};

use crate::error::AptuError;

use super::{AiConfig, CacheConfig, ReposConfig, ReviewConfig};

/// Trait for loading application configuration from any source.
///
/// Decouples configuration loading from the filesystem, enabling
/// file-based (TOML), in-memory (test/WASM), and future sources
/// (e.g., iOS plist, remote config) to implement this trait.
pub trait ConfigSource: Send + Sync {
    /// Load and return the application configuration.
    ///
    /// # Errors
    ///
    /// Returns `AptuError::Config` if the source contains invalid data.
    fn load(&self) -> Result<AppConfig, AptuError>;
}

/// In-memory configuration source for testing and WASM environments.
///
/// Holds a pre-built `AppConfig` and returns a clone on `load()`.
/// Always available (no cfg gate).
pub struct InMemoryConfigSource(pub AppConfig);

impl ConfigSource for InMemoryConfigSource {
    fn load(&self) -> Result<AppConfig, AptuError> {
        Ok(self.0.clone())
    }
}

/// TOML file-based configuration source.
///
/// Reads from the standard `config.toml` file in the XDG config directory
/// and overlays environment variables with the `APTU_` prefix.
#[cfg(not(target_arch = "wasm32"))]
pub struct TomlConfigSource;

#[cfg(not(target_arch = "wasm32"))]
impl TomlConfigSource {
    /// Create a new TOML config source.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for TomlConfigSource {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ConfigSource for TomlConfigSource {
    fn load(&self) -> Result<AppConfig, AptuError> {
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

        // Validate cache configuration
        app_config
            .cache
            .validate()
            .map_err(|e| AptuError::Config { message: e })?;

        // Validate review configuration consistency at load time (non-fatal warnings).
        for warning in app_config.review.validate_consistency() {
            tracing::warn!("{}", warning);
        }

        Ok(app_config)
    }
}

/// User preferences.
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct UserConfig {
    /// Default repository to use (skip repo selection).
    pub default_repo: Option<String>,
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

/// Per-field byte limits for user-supplied content before prompt assembly.
/// These limits defend against prompt injection by enforcing a hard cap on
/// how much user-controlled data can reach the AI model.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct PromptConfig {
    /// Maximum bytes for an issue body (default: 32 KiB).
    ///
    /// Limits the size of user-supplied issue body text before it is wrapped
    /// in XML tags and sent to the AI model. Larger limits allow more context
    /// but increase token usage and prompt injection surface area. The default
    /// (32 KiB) balances context richness against cost and security.
    pub max_issue_body_bytes: usize,
    /// Maximum bytes for a PR diff (default: 512 KiB).
    ///
    /// Limits the total size of all file patches in a PR before they are
    /// wrapped in XML tags and sent to the AI model. Raised from 128 KiB
    /// to 512 KiB to accommodate large refactor PRs; injection defence is
    /// provided by XML tag stripping which is independent of diff size.
    pub max_diff_bytes: usize,
    /// Maximum bytes for a commit message (default: 4 KiB).
    ///
    /// Limits the size of commit message text before wrapping. The default
    /// (4 KiB) is conservative, as commit messages are typically short;
    /// this prevents abuse via artificially large commit messages.
    pub max_commit_message_bytes: usize,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            max_issue_body_bytes: 32_768,
            max_diff_bytes: 524_288,
            max_commit_message_bytes: 4_096,
        }
    }
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
    /// Prompt injection defence settings.
    #[serde(default)]
    pub prompt: PromptConfig,
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
/// This is a convenience shim that delegates to [`TomlConfigSource`].
///
/// # Errors
///
/// Returns `AptuError::Config` if the config file exists but is invalid.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_config() -> Result<AppConfig, AptuError> {
    TomlConfigSource::new().load()
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
        assert_eq!(config.ai.model, super::super::ai::DEFAULT_OPENROUTER_MODEL);
        assert_eq!(config.ai.timeout_seconds, 30);
        assert_eq!(config.ai.max_tokens, 4096);
        assert!(config.ai.allow_paid_models);
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
model = "gemini-3.1-flash-lite"

[ai.tasks.triage]
model = "gemini-3.1-flash-lite"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert_eq!(app_config.ai.provider, "gemini");
        assert_eq!(app_config.ai.model, super::super::ai::DEFAULT_GEMINI_MODEL);
        assert!(app_config.ai.tasks.is_some());

        let tasks = app_config.ai.tasks.unwrap();
        assert!(tasks.triage.is_some());
        assert!(tasks.review.is_none());
        assert!(tasks.create.is_none());

        let triage = tasks.triage.unwrap();
        assert_eq!(triage.provider, None);
        assert_eq!(
            triage.model,
            Some(super::super::ai::DEFAULT_GEMINI_MODEL.to_string())
        );
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
        assert_eq!(
            triage.model,
            Some(super::super::ai::DEFAULT_OPENROUTER_MODEL.to_string())
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
            Some("anthropic/claude-sonnet-4.6".to_string())
        );
    }

    #[test]
    fn test_config_with_partial_task_overrides() {
        // Test that partial task configs (only provider or only model) parse correctly
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite"

[ai.tasks.triage]
provider = "gemini"

[ai.tasks.review]
model = "gemini-3.1-flash-lite"
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
        assert_eq!(
            review.model,
            Some(super::super::ai::DEFAULT_GEMINI_MODEL.to_string())
        );
    }

    #[test]
    fn test_config_without_tasks_section() {
        // Test that config without explicit tasks section uses defaults
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert_eq!(app_config.ai.provider, "gemini");
        assert_eq!(app_config.ai.model, super::super::ai::DEFAULT_GEMINI_MODEL);
        // When no tasks section is provided, defaults are used (tasks: None)
        assert!(app_config.ai.tasks.is_none());
    }

    #[test]
    fn test_resolve_for_task_with_defaults() {
        // Test that resolve_for_task returns correct defaults (all tasks use openrouter)
        let ai_config = AiConfig::default();

        // All tasks use global defaults (openrouter/mistralai/mistral-small-2603)
        let (provider, model) = ai_config.resolve_for_task(super::super::ai::TaskType::Triage);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, super::super::ai::DEFAULT_OPENROUTER_MODEL);
        assert!(ai_config.allow_paid_models);

        let (provider, model) = ai_config.resolve_for_task(super::super::ai::TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, super::super::ai::DEFAULT_OPENROUTER_MODEL);
        assert!(ai_config.allow_paid_models);

        let (provider, model) = ai_config.resolve_for_task(super::super::ai::TaskType::Create);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "mistralai/mistral-small-2603");
        assert!(ai_config.allow_paid_models);
    }

    #[test]
    fn test_resolve_for_task_with_triage_override() {
        // Test that resolve_for_task returns triage override when present
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite"

[ai.tasks.triage]
model = "gemini-3.1-flash-lite"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Triage should use override
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Triage);
        assert_eq!(provider, "gemini");
        assert_eq!(model, super::super::ai::DEFAULT_GEMINI_MODEL);

        // Review and Create should use defaults
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Review);
        assert_eq!(provider, "gemini");
        assert_eq!(model, super::super::ai::DEFAULT_GEMINI_MODEL);

        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, super::super::ai::DEFAULT_GEMINI_MODEL);
    }

    #[test]
    fn test_config_with_provider_override() {
        // Test that resolve_for_task returns provider override when present
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite"

[ai.tasks.review]
provider = "openrouter"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Review should use provider override but default model
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, super::super::ai::DEFAULT_GEMINI_MODEL);

        // Triage and Create should use defaults
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Triage);
        assert_eq!(provider, "gemini");
        assert_eq!(model, super::super::ai::DEFAULT_GEMINI_MODEL);

        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, super::super::ai::DEFAULT_GEMINI_MODEL);
    }

    #[test]
    fn test_config_with_full_overrides() {
        // Test that resolve_for_task returns both provider and model overrides
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite"

[ai.tasks.triage]
provider = "openrouter"
model = "mistralai/mistral-small-2603"

[ai.tasks.review]
provider = "openrouter"
model = "anthropic/claude-haiku-4.5"

[ai.tasks.create]
provider = "gemini"
model = "gemini-3.1-flash-lite"
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        // Triage
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Triage);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, super::super::ai::DEFAULT_OPENROUTER_MODEL);

        // Review
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, "anthropic/claude-haiku-4.5");

        // Create
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Create);
        assert_eq!(provider, "gemini");
        assert_eq!(model, super::super::ai::DEFAULT_GEMINI_MODEL);
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
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Triage);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, super::super::ai::DEFAULT_OPENROUTER_MODEL);

        // Review: provider override, model from default
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Review);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, super::super::ai::DEFAULT_OPENROUTER_MODEL);

        // Create: empty override, both from default
        let (provider, model) = app_config
            .ai
            .resolve_for_task(super::super::ai::TaskType::Create);
        assert_eq!(provider, "openrouter");
        assert_eq!(model, super::super::ai::DEFAULT_OPENROUTER_MODEL);
    }

    #[test]
    fn test_fallback_config_toml_parsing() {
        // Test that FallbackConfig deserializes from TOML correctly
        let config_str = r#"
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite"

[ai.fallback]
chain = ["openrouter", "anthropic"]
"#;

        let config = Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Toml))
            .build()
            .expect("should build config");

        let app_config: AppConfig = config.try_deserialize().expect("should deserialize");

        assert_eq!(app_config.ai.provider, "gemini");
        assert_eq!(app_config.ai.model, "gemini-3.1-flash-lite");
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
model = "gemini-3.1-flash-lite"

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
model = "gemini-3.1-flash-lite"

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
model = "gemini-3.1-flash-lite"
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

    #[test]
    fn test_review_config_defaults() {
        // Arrange / Act: construct ReviewConfig with defaults
        let review_config = ReviewConfig::default();

        // Assert: defaults match specification
        assert_eq!(
            review_config.max_prompt_chars, 120_000,
            "max_prompt_chars should default to 120_000"
        );
        assert_eq!(
            review_config.max_full_content_files, 10,
            "max_full_content_files should default to 10"
        );
        assert_eq!(
            review_config.max_chars_per_file, 16_000,
            "max_chars_per_file should default to 16_000"
        );

        // Assert: AppConfig::default().review equals ReviewConfig::default()
        let app_config = AppConfig::default();
        assert_eq!(
            app_config.review.max_prompt_chars, review_config.max_prompt_chars,
            "AppConfig review defaults should match ReviewConfig defaults"
        );
        assert_eq!(
            app_config.review.max_full_content_files, review_config.max_full_content_files,
            "AppConfig review defaults should match ReviewConfig defaults"
        );
        assert_eq!(
            app_config.review.max_chars_per_file, review_config.max_chars_per_file,
            "AppConfig review defaults should match ReviewConfig defaults"
        );
    }

    #[test]
    fn test_in_memory_config_source_loads_defaults() {
        let default_config = AppConfig::default();
        let source = InMemoryConfigSource(default_config.clone());
        let loaded = source.load().expect("load should succeed");
        assert_eq!(loaded.ai.provider, default_config.ai.provider);
        assert_eq!(loaded.ai.model, default_config.ai.model);
        assert_eq!(loaded.ai.timeout_seconds, default_config.ai.timeout_seconds);
        assert_eq!(loaded.ai.max_tokens, default_config.ai.max_tokens);
        assert_eq!(
            loaded.github.api_timeout_seconds,
            default_config.github.api_timeout_seconds
        );
    }
}
