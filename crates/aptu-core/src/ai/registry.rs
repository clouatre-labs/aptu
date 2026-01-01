// SPDX-License-Identifier: Apache-2.0

//! Centralized provider configuration registry.
//!
//! This module provides a static registry of all AI providers supported by Aptu,
//! including their metadata, API endpoints, and available models.
//!
//! It also provides runtime model validation infrastructure via the `ModelRegistry` trait
//! and `CachedModelRegistry` implementation for fetching live model lists from provider APIs.
//!
//! # Examples
//!
//! ```
//! use aptu_core::ai::registry::{get_provider, all_providers};
//!
//! // Get a specific provider
//! let provider = get_provider("openrouter");
//! assert!(provider.is_some());
//!
//! // Get all providers
//! let providers = all_providers();
//! assert_eq!(providers.len(), 6);
//! ```

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Metadata for a single AI model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    /// Human-readable model name for UI display
    pub display_name: &'static str,

    /// Provider-specific model identifier used in API requests
    pub identifier: &'static str,

    /// Whether this model is free to use
    pub is_free: bool,

    /// Maximum context window size in tokens
    pub context_window: u32,
}

/// Configuration for an AI provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderConfig {
    /// Provider identifier (lowercase, used in config files)
    pub name: &'static str,

    /// Human-readable provider name for UI display
    pub display_name: &'static str,

    /// API base URL for this provider
    pub api_url: &'static str,

    /// Environment variable name for API key
    pub api_key_env: &'static str,

    /// Available models for this provider
    pub models: &'static [ModelInfo],
}

// ============================================================================
// Provider Models
// ============================================================================

/// `Gemini` models
const GEMINI_MODELS: &[ModelInfo] = &[ModelInfo {
    display_name: "Gemini 3 Flash",
    identifier: "gemini-3-flash-preview",
    is_free: true,
    context_window: 1_048_576,
}];

/// `OpenRouter` models
const OPENROUTER_MODELS: &[ModelInfo] = &[
    ModelInfo {
        display_name: "Devstral 2",
        identifier: "mistralai/devstral-2512:free",
        is_free: true,
        context_window: 262_144,
    },
    ModelInfo {
        display_name: "Claude Haiku 4.5",
        identifier: "anthropic/claude-haiku-4.5",
        is_free: false,
        context_window: 200_000,
    },
];

/// `Groq` models
const GROQ_MODELS: &[ModelInfo] = &[ModelInfo {
    display_name: "GPT-OSS 20B",
    identifier: "openai/gpt-oss-20b",
    is_free: true,
    context_window: 131_072,
}];

/// `Cerebras` models
const CEREBRAS_MODELS: &[ModelInfo] = &[ModelInfo {
    display_name: "Llama 3.3 70B",
    identifier: "llama-3.3-70b",
    is_free: true,
    context_window: 128_000,
}];

/// `Zenmux` models
const ZENMUX_MODELS: &[ModelInfo] = &[ModelInfo {
    display_name: "Grok Code Fast 1",
    identifier: "x-ai/grok-code-fast-1",
    is_free: true,
    context_window: 256_000,
}];

/// `Z.AI` models
const ZAI_MODELS: &[ModelInfo] = &[ModelInfo {
    display_name: "GLM-4.5 Air",
    identifier: "glm-4.5-air",
    is_free: false,
    context_window: 128_000,
}];

// ============================================================================
// Provider Registry
// ============================================================================

/// Static registry of all supported AI providers
pub static PROVIDERS: &[ProviderConfig] = &[
    ProviderConfig {
        name: "gemini",
        display_name: "Google Gemini",
        api_url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        api_key_env: "GEMINI_API_KEY",
        models: GEMINI_MODELS,
    },
    ProviderConfig {
        name: "openrouter",
        display_name: "OpenRouter",
        api_url: "https://openrouter.ai/api/v1/chat/completions",
        api_key_env: "OPENROUTER_API_KEY",
        models: OPENROUTER_MODELS,
    },
    ProviderConfig {
        name: "groq",
        display_name: "Groq",
        api_url: "https://api.groq.com/openai/v1/chat/completions",
        api_key_env: "GROQ_API_KEY",
        models: GROQ_MODELS,
    },
    ProviderConfig {
        name: "cerebras",
        display_name: "Cerebras",
        api_url: "https://api.cerebras.ai/v1/chat/completions",
        api_key_env: "CEREBRAS_API_KEY",
        models: CEREBRAS_MODELS,
    },
    ProviderConfig {
        name: "zenmux",
        display_name: "Zenmux",
        api_url: "https://zenmux.ai/api/v1/chat/completions",
        api_key_env: "ZENMUX_API_KEY",
        models: ZENMUX_MODELS,
    },
    ProviderConfig {
        name: "zai",
        display_name: "Z.AI (Zhipu)",
        api_url: "https://api.z.ai/api/paas/v4/chat/completions",
        api_key_env: "ZAI_API_KEY",
        models: ZAI_MODELS,
    },
];

/// Retrieves a provider configuration by name.
///
/// # Arguments
///
/// * `name` - The provider name (case-sensitive, lowercase)
///
/// # Returns
///
/// Some(ProviderConfig) if found, None otherwise.
///
/// # Examples
///
/// ```
/// use aptu_core::ai::registry::get_provider;
///
/// let provider = get_provider("openrouter");
/// assert!(provider.is_some());
/// assert_eq!(provider.unwrap().display_name, "OpenRouter");
/// ```
#[must_use]
pub fn get_provider(name: &str) -> Option<&'static ProviderConfig> {
    PROVIDERS.iter().find(|p| p.name == name)
}

/// Returns all available providers.
///
/// # Returns
///
/// A slice of all `ProviderConfig` entries in the registry.
///
/// # Examples
///
/// ```
/// use aptu_core::ai::registry::all_providers;
///
/// let providers = all_providers();
/// assert_eq!(providers.len(), 6);
/// ```
#[must_use]
pub fn all_providers() -> &'static [ProviderConfig] {
    PROVIDERS
}

// ============================================================================
// Runtime Model Registry
// ============================================================================

/// A model from a provider's API response.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeModel {
    /// Provider-specific model identifier
    pub id: String,
    /// Human-readable model name
    pub name: String,
    /// Whether this model is free to use
    pub is_free: bool,
    /// Maximum context window size in tokens (if available)
    pub context_window: Option<u32>,
}

/// Trait for runtime model validation and discovery.
///
/// Provides methods to fetch live model lists from provider APIs,
/// validate model existence, and suggest similar models.
#[async_trait]
pub trait ModelRegistry: Send + Sync {
    /// Fetch all available models from the provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider name (e.g., "openrouter", "gemini")
    ///
    /// # Returns
    ///
    /// A vector of available models, or an error if the API call fails.
    async fn list_models(&self, provider: &str) -> Result<Vec<RuntimeModel>>;

    /// Check if a model exists for the given provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider name
    /// * `model_id` - Model identifier to check
    ///
    /// # Returns
    ///
    /// `true` if the model exists, `false` otherwise.
    async fn model_exists(&self, provider: &str, model_id: &str) -> Result<bool>;

    /// Suggest similar models based on a partial identifier.
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider name
    /// * `partial_id` - Partial model identifier to match
    ///
    /// # Returns
    ///
    /// A vector of matching models, or an error if the API call fails.
    async fn suggest_similar(&self, provider: &str, partial_id: &str) -> Result<Vec<RuntimeModel>>;
}

/// Cached model registry with TTL-based expiration.
///
/// Fetches model lists from provider APIs and caches them locally
/// with a configurable TTL (default 24 hours).
#[allow(dead_code)]
pub struct CachedModelRegistry {
    /// Cache directory path
    cache_dir: PathBuf,
    /// TTL in seconds (default 86400 = 24 hours)
    ttl_seconds: u64,
}

impl CachedModelRegistry {
    /// Create a new cached model registry.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory to store cached model lists
    /// * `ttl_seconds` - Time-to-live for cache entries in seconds
    #[must_use]
    pub fn new(cache_dir: PathBuf, ttl_seconds: u64) -> Self {
        Self {
            cache_dir,
            ttl_seconds,
        }
    }

    /// Create a new cached model registry with default TTL (24 hours).
    #[must_use]
    pub fn with_default_ttl(cache_dir: PathBuf) -> Self {
        Self::new(cache_dir, 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_provider_gemini() {
        let provider = get_provider("gemini");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Google Gemini");
        assert_eq!(provider.api_key_env, "GEMINI_API_KEY");
        assert!(!provider.models.is_empty());
    }

    #[test]
    fn test_get_provider_openrouter() {
        let provider = get_provider("openrouter");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "OpenRouter");
        assert_eq!(provider.api_key_env, "OPENROUTER_API_KEY");
        assert!(!provider.models.is_empty());
    }

    #[test]
    fn test_get_provider_groq() {
        let provider = get_provider("groq");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Groq");
        assert_eq!(provider.api_key_env, "GROQ_API_KEY");
        assert!(!provider.models.is_empty());
    }

    #[test]
    fn test_get_provider_cerebras() {
        let provider = get_provider("cerebras");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Cerebras");
        assert_eq!(provider.api_key_env, "CEREBRAS_API_KEY");
        assert!(!provider.models.is_empty());
    }

    #[test]
    fn test_get_provider_not_found() {
        let provider = get_provider("nonexistent");
        assert!(provider.is_none());
    }

    #[test]
    fn test_get_provider_case_sensitive() {
        let provider = get_provider("OpenRouter");
        assert!(
            provider.is_none(),
            "Provider lookup should be case-sensitive"
        );
    }

    #[test]
    fn test_all_providers_count() {
        let providers = all_providers();
        assert_eq!(providers.len(), 6, "Should have exactly 6 providers");
    }

    #[test]
    fn test_all_providers_have_models() {
        let providers = all_providers();
        for provider in providers {
            assert!(
                !provider.models.is_empty(),
                "Provider {} should have at least one model",
                provider.name
            );
        }
    }

    #[test]
    fn test_all_providers_have_unique_names() {
        let providers = all_providers();
        let mut names = Vec::new();
        for provider in providers {
            assert!(
                !names.contains(&provider.name),
                "Duplicate provider name: {}",
                provider.name
            );
            names.push(provider.name);
        }
    }

    #[test]
    fn test_gemini_models() {
        let provider = get_provider("gemini").unwrap();
        assert_eq!(provider.models.len(), 1);
        let model = &provider.models[0];
        assert_eq!(model.identifier, "gemini-3-flash-preview");
        assert!(model.is_free);
    }

    #[test]
    fn test_openrouter_models() {
        let provider = get_provider("openrouter").unwrap();
        assert_eq!(provider.models.len(), 2);
        let free_models: Vec<_> = provider.models.iter().filter(|m| m.is_free).collect();
        assert!(
            !free_models.is_empty(),
            "OpenRouter should have free models"
        );
    }

    #[test]
    fn test_groq_models() {
        let provider = get_provider("groq").unwrap();
        assert!(!provider.models.is_empty());
        let model = &provider.models[0];
        assert_eq!(model.identifier, "openai/gpt-oss-20b");
    }

    #[test]
    fn test_cerebras_models() {
        let provider = get_provider("cerebras").unwrap();
        assert!(!provider.models.is_empty());
    }

    #[test]
    fn test_get_provider_zenmux() {
        let provider = get_provider("zenmux");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Zenmux");
        assert_eq!(provider.api_key_env, "ZENMUX_API_KEY");
        assert!(!provider.models.is_empty());
    }

    #[test]
    fn test_get_provider_zai() {
        let provider = get_provider("zai");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Z.AI (Zhipu)");
        assert_eq!(provider.api_key_env, "ZAI_API_KEY");
        assert!(!provider.models.is_empty());
    }

    #[test]
    fn test_zenmux_models() {
        let provider = get_provider("zenmux").unwrap();
        assert_eq!(provider.models.len(), 1);
        let model = &provider.models[0];
        assert_eq!(model.identifier, "x-ai/grok-code-fast-1");
        assert!(model.is_free);
        assert_eq!(model.context_window, 256_000);
    }

    #[test]
    fn test_zai_models() {
        let provider = get_provider("zai").unwrap();
        assert_eq!(provider.models.len(), 1);
        let model = &provider.models[0];
        assert_eq!(model.identifier, "glm-4.5-air");
        assert!(!model.is_free);
        assert_eq!(model.context_window, 128_000);
    }

    #[test]
    fn test_model_identifiers_unique_within_provider() {
        let providers = all_providers();
        for provider in providers {
            let mut identifiers = Vec::new();
            for model in provider.models {
                assert!(
                    !identifiers.contains(&model.identifier),
                    "Duplicate model identifier in {}: {}",
                    provider.name,
                    model.identifier
                );
                identifiers.push(model.identifier);
            }
        }
    }

    #[test]
    fn test_provider_api_urls_valid() {
        let providers = all_providers();
        for provider in providers {
            assert!(
                provider.api_url.starts_with("https://"),
                "Provider {} API URL should use HTTPS",
                provider.name
            );
        }
    }

    #[test]
    fn test_provider_api_key_env_not_empty() {
        let providers = all_providers();
        for provider in providers {
            assert!(
                !provider.api_key_env.is_empty(),
                "Provider {} should have API key env var",
                provider.name
            );
        }
    }
}
