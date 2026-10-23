// SPDX-License-Identifier: Apache-2.0

//! AI provider configuration.

use std::collections::BTreeMap;

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
    /// Findings verification task (PR review post-processing).
    Verify,
}

/// Task-specific AI model override.
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct TaskOverride {
    /// Optional provider override for this task.
    pub provider: Option<String>,
    /// Optional model override for this task.
    pub model: Option<String>,
    /// Optional small model for routing (used when `estimated_size` < `routing_threshold_chars`).
    #[serde(default)]
    pub small_model: Option<String>,
    /// Optional large model for routing (used when `estimated_size` >= `routing_threshold_chars`).
    #[serde(default)]
    pub large_model: Option<String>,
    /// Optional threshold for routing between `small_model` and `large_model` (default: 60000 for review).
    #[serde(default)]
    pub routing_threshold_chars: Option<usize>,
    /// Optional request timeout override in seconds for this task.
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
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
    /// Findings verification task configuration.
    pub verify: Option<TaskOverride>,
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
    /// AI provider: one of `"gemini"`, `"openrouter"`, `"zai"`, or `"anthropic"`.
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
    /// `OpenRouter` data collection setting: `"allow"` or `"deny"` (default: `"deny"`).
    pub openrouter_data_collection: String,
    /// `OpenRouter` Zero Data Retention requirement (default: `true`).
    pub openrouter_zdr: bool,
    /// Model context windows (tokens), keyed by model identifier.
    ///
    /// Overrides the built-in defaults for prompt budgeting. Models absent
    /// from both this map and the built-in defaults resolve to `None`
    /// (treated as unbounded), which preserves the historical behavior for
    /// providers that advertise no window.
    pub context_windows: Option<BTreeMap<String, u32>>,
}

/// Built-in context-window defaults for aptu's default models.
///
/// `mistralai/mistral-small-2603` = 262144 is verified against the live
/// `OpenRouter` API. Default models without a verified window (e.g. the
/// `Gemini` default) are deliberately absent: lookups degrade to `None`
/// (unbounded) with a debug log rather than guessing a value.
fn builtin_context_windows() -> BTreeMap<String, u32> {
    BTreeMap::from([(DEFAULT_OPENROUTER_MODEL.to_string(), 262_144)])
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
            retry_max_attempts: default_retry_max_attempts(),
            tasks: None,
            fallback: None,
            custom_guidance: None,
            validation_enabled: true,
            openrouter_data_collection: "deny".to_string(),
            openrouter_zdr: true,
            context_windows: None,
        }
    }
}

/// Fold a list of optional context windows into their minimum, conservatively
/// excluding models with no window. Returns the minimum and the number of
/// models that resolved (advertised a window).
#[must_use]
pub fn fold_context_windows(windows: &[Option<u32>]) -> (Option<u32>, usize) {
    let mut min_window: Option<u32> = None;
    let mut resolved = 0;
    for window in windows.iter().flatten() {
        resolved += 1;
        min_window = Some(min_window.map_or(*window, |cur| cur.min(*window)));
    }
    (min_window, resolved)
}

impl AiConfig {
    /// Look up the context window (tokens) for `model`.
    ///
    /// Resolution order: user config (`context_windows`) -> built-in defaults
    /// -> `None` (unbounded). Pure string lookup; no network access. A miss
    /// for one of aptu's built-in default models logs a debug message so
    /// mis-sized budgets are visible in trace output.
    #[must_use]
    pub fn context_window_for(&self, model: &str) -> Option<u32> {
        if let Some(window) = self
            .context_windows
            .as_ref()
            .and_then(|map| map.get(model).copied())
        {
            return Some(window);
        }
        if let Some(window) = builtin_context_windows().get(model).copied() {
            return Some(window);
        }
        if model == DEFAULT_OPENROUTER_MODEL || model == DEFAULT_GEMINI_MODEL {
            tracing::debug!(
                model = model,
                "Default model has no built-in context window; budget treated as unbounded"
            );
        }
        None
    }

    /// Resolve the effective context window across the selected model and the
    /// fallback chain.
    ///
    /// Semantics are intentionally conservative: a model with no configured
    /// window is EXCLUDED from the minimum rather than constraining it (an
    /// unknown model means unbounded, which matches the historical behavior
    /// for providers that advertise no window). Only when no model at all
    /// resolves does this degrade to `None` (unbounded), with a debug log.
    #[must_use]
    pub fn effective_context_window(&self, provider_name: &str, model_name: &str) -> Option<u32> {
        let mut targets: Vec<(&str, &str)> = vec![(provider_name, model_name)];
        if let Some(fallback) = &self.fallback {
            for entry in &fallback.chain {
                targets.push((
                    entry.provider.as_str(),
                    entry.model.as_deref().unwrap_or(self.model.as_str()),
                ));
            }
        }

        let windows: Vec<Option<u32>> = targets
            .iter()
            .map(|(_, model)| self.context_window_for(model))
            .collect();
        let (min_window, resolved) = fold_context_windows(&windows);
        if resolved == 0 {
            tracing::debug!(
                provider = provider_name,
                model = model_name,
                "No configured context window for selected model or fallback chain; budget treated as unbounded"
            );
        }
        min_window
    }

    /// Resolve provider and model for a specific task type.
    ///
    /// Returns a tuple of (provider, model) by checking task-specific overrides first,
    /// then falling back to the default provider and model. When `estimated_size` is provided
    /// and routing fields (`small_model`/`large_model`) are configured, selects between
    /// small and large models based on the routing threshold.
    ///
    /// # Arguments
    ///
    /// * `task` - The task type to resolve configuration for
    /// * `estimated_size` - Optional estimated size of the prompt in characters (used for routing)
    ///
    /// # Returns
    ///
    /// A tuple of (`provider_name`, `model_name`) strings
    #[must_use]
    pub fn resolve_for_task(
        &self,
        task: TaskType,
        estimated_size: Option<usize>,
    ) -> (String, String) {
        let task_override = self.task_override_for(task);

        let provider = task_override
            .and_then(|o| o.provider.clone())
            .unwrap_or_else(|| self.provider.clone());

        // Branch 1: Explicit model override bypasses routing entirely
        if let Some(model) = task_override.and_then(|o| o.model.clone()) {
            return (provider, model);
        }

        // Branch 2: Both routing fields set and estimated_size available
        if let (Some(small), Some(large), Some(size)) = (
            task_override.and_then(|o| o.small_model.clone()),
            task_override.and_then(|o| o.large_model.clone()),
            estimated_size,
        ) {
            let default_threshold = match task {
                TaskType::Review => 60_000,
                TaskType::Triage | TaskType::Create | TaskType::Verify => 8_192,
            };
            let threshold = task_override
                .and_then(|o| o.routing_threshold_chars)
                .unwrap_or(default_threshold);
            if size < threshold {
                return (provider.clone(), small);
            }
            return (provider, large);
        }

        // Branch 3: Exactly one of small_model/large_model set - warn and fall through
        let has_small = task_override.and_then(|o| o.small_model.as_ref()).is_some();
        let has_large = task_override.and_then(|o| o.large_model.as_ref()).is_some();
        if has_small != has_large {
            let missing = if has_small {
                "large_model"
            } else {
                "small_model"
            };
            tracing::warn!(
                "TaskOverride has only one routing field set (missing {missing}); falling back to default model"
            );
        }

        // Branch 4: Fallback to self.model
        let model = self.model.clone();
        (provider, model)
    }

    /// Return the per-task override for the given task, if configured.
    fn task_override_for(&self, task: TaskType) -> Option<&TaskOverride> {
        let tasks = self.tasks.as_ref()?;
        match task {
            TaskType::Triage => tasks.triage.as_ref(),
            TaskType::Review => tasks.review.as_ref(),
            TaskType::Create => tasks.create.as_ref(),
            TaskType::Verify => tasks.verify.as_ref(),
        }
    }

    /// Resolve the effective request timeout in seconds for a specific task.
    ///
    /// Resolution order:
    /// 1. Explicit per-task override (`[ai.tasks.<task>].timeout_seconds`,
    ///    treated as unset when zero)
    /// 2. Per-task default headroom: review = 120 seconds
    /// 3. Global `ai.timeout_seconds` (default 30) for triage and create
    ///
    /// A timeout of `0` is treated as unset at every step, so a misconfigured
    /// zero value falls through to the next resolution step instead of
    /// producing an immediately-timing-out client.
    #[must_use]
    pub fn effective_timeout_for_task(&self, task: TaskType) -> u64 {
        let task_override = self.task_override_for(task);

        if let Some(timeout) = task_override
            .and_then(|o| o.timeout_seconds)
            .filter(|t| *t > 0)
        {
            return timeout;
        }

        match task {
            // Review operations get extra headroom by default since large
            // diffs take significantly longer to analyze.
            TaskType::Review => 120,
            TaskType::Triage | TaskType::Create | TaskType::Verify => {
                if self.timeout_seconds > 0 {
                    self.timeout_seconds
                } else {
                    30
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_timeout_defaults() {
        let config = AiConfig::default();
        assert_eq!(config.effective_timeout_for_task(TaskType::Review), 120);
        assert_eq!(config.effective_timeout_for_task(TaskType::Triage), 30);
        assert_eq!(config.effective_timeout_for_task(TaskType::Create), 30);
        assert_eq!(config.effective_timeout_for_task(TaskType::Verify), 30);
    }

    #[test]
    fn test_effective_timeout_explicit_override_wins() {
        let config = AiConfig {
            tasks: Some(TasksConfig {
                review: Some(TaskOverride {
                    timeout_seconds: Some(240),
                    ..TaskOverride::default()
                }),
                ..TasksConfig::default()
            }),
            ..AiConfig::default()
        };
        assert_eq!(config.effective_timeout_for_task(TaskType::Review), 240);
    }

    #[test]
    fn test_effective_timeout_global_honored_for_triage_and_create() {
        let config = AiConfig {
            timeout_seconds: 45,
            ..AiConfig::default()
        };
        assert_eq!(config.effective_timeout_for_task(TaskType::Triage), 45);
        assert_eq!(config.effective_timeout_for_task(TaskType::Create), 45);
        // Review keeps its 120s headroom even with an explicit global timeout.
        assert_eq!(config.effective_timeout_for_task(TaskType::Review), 120);
    }

    #[test]
    fn test_effective_timeout_zero_is_treated_as_unset() {
        let config = AiConfig {
            timeout_seconds: 0,
            tasks: Some(TasksConfig {
                triage: Some(TaskOverride {
                    timeout_seconds: Some(0),
                    ..TaskOverride::default()
                }),
                ..TasksConfig::default()
            }),
            ..AiConfig::default()
        };
        // Zero override and zero global both fall through to task defaults.
        assert_eq!(config.effective_timeout_for_task(TaskType::Triage), 30);
        assert_eq!(config.effective_timeout_for_task(TaskType::Review), 120);
    }

    #[test]
    fn test_resolve_for_task_verify_uses_small_model_tier() {
        let config = AiConfig {
            provider: "gemini".to_string(),
            model: "default-model".to_string(),
            tasks: Some(TasksConfig {
                verify: Some(TaskOverride {
                    small_model: Some("small-verify".to_string()),
                    large_model: Some("large-verify".to_string()),
                    ..TaskOverride::default()
                }),
                ..TasksConfig::default()
            }),
            ..AiConfig::default()
        };

        // Small prompt routes to the small tier; large prompt to the large tier.
        let (provider, model) = config.resolve_for_task(TaskType::Verify, Some(1_000));
        assert_eq!(provider, "gemini");
        assert_eq!(model, "small-verify");
        let (_, large_model) = config.resolve_for_task(TaskType::Verify, Some(10_000));
        assert_eq!(large_model, "large-verify");
    }

    #[test]
    fn test_context_window_defaults_and_unknown_models() {
        let config = AiConfig::default();

        // Zero-config happy path: the verified OpenRouter default resolves.
        assert_eq!(
            config.context_window_for(DEFAULT_OPENROUTER_MODEL),
            Some(262_144)
        );

        // Unknown strings and models whose window we could not verify
        // (Gemini default, anthropic, zai) degrade to None -- identical to
        // the historical behavior where those providers advertised no window.
        assert_eq!(config.context_window_for("totally-unknown-model"), None);
        assert_eq!(config.context_window_for(DEFAULT_GEMINI_MODEL), None);
        assert_eq!(config.context_window_for("claude-sonnet-4"), None);
        assert_eq!(config.context_window_for("glm-4.7"), None);
    }

    #[test]
    fn test_context_window_user_config_overrides_builtin() {
        let config = AiConfig {
            context_windows: Some(BTreeMap::from([(
                DEFAULT_OPENROUTER_MODEL.to_string(),
                1_000,
            )])),
            ..AiConfig::default()
        };
        assert_eq!(
            config.context_window_for(DEFAULT_OPENROUTER_MODEL),
            Some(1_000)
        );
    }

    #[test]
    fn test_effective_context_window_takes_min_across_fallback_chain() {
        let config = AiConfig {
            fallback: Some(FallbackConfig {
                chain: vec![FallbackEntry {
                    provider: "openrouter".to_string(),
                    model: Some("tiny-model".to_string()),
                }],
            }),
            context_windows: Some(BTreeMap::from([
                ("big-model".to_string(), 200_000),
                ("tiny-model".to_string(), 8_000),
                ("missing-window".to_string(), 4_000),
            ])),
            ..AiConfig::default()
        };

        // Min across the chain; a fallback with no configured window
        // ("unset-window") is excluded from the minimum, not a constraint.
        assert_eq!(
            config.effective_context_window("openrouter", "big-model"),
            Some(8_000)
        );

        // No model resolves -> unbounded (None). No fallback chain so only
        // the selected model is considered (also proves anthropic models
        // resolve None today, unchanged from the registry era).
        let bare = AiConfig::default();
        assert_eq!(
            bare.effective_context_window("anthropic", "claude-unknown"),
            None
        );
    }

    #[test]
    fn test_fold_context_windows_excludes_unresolved_and_takes_min() {
        let windows = [Some(200_000), None, Some(8_000)];

        // Unresolved (None) entries are excluded; min is over resolved only.
        let (min_window, resolved) = fold_context_windows(&windows);
        assert_eq!(resolved, 2, "unresolved entries must not count as resolved");
        assert_eq!(min_window, Some(8_000), "min must be over resolved windows");

        // No model resolves -> no minimum (degrades to unbounded upstream).
        let (none_min, none_resolved) = fold_context_windows(&[None, None]);
        assert_eq!(none_resolved, 0);
        assert_eq!(none_min, None);
    }
}
