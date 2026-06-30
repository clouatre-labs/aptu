// SPDX-License-Identifier: Apache-2.0

//! AI provider configuration.

use serde::{Deserialize, Serialize};

/// Default `OpenRouter` model identifier.
pub const DEFAULT_OPENROUTER_MODEL: &str = "mistralai/mistral-small-2603";
/// Default `Gemini` model identifier.
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3.1-flash-lite";

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
    /// AI provider: one of `"gemini"`, `"openrouter"`, `"groq"`, `"cerebras"`, `"zenmux"`, or `"zai"`.
    pub provider: String,
    /// Model identifier.
    pub model: String,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Allow paid models (default: true).
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
