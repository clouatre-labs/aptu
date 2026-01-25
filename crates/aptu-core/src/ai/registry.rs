// SPDX-License-Identifier: Apache-2.0

//! Centralized provider configuration registry.
//!
//! This module provides a static registry of all AI providers supported by Aptu,
//! including their metadata, API endpoints, and available models.
//!
//! It also provides runtime model validation infrastructure via the `ModelRegistry` trait
//! with a simple sync implementation using static model lists.
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

use async_trait::async_trait;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

use crate::auth::TokenProvider;
use crate::cache::FileCache;

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
}

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
    },
    ProviderConfig {
        name: "openrouter",
        display_name: "OpenRouter",
        api_url: "https://openrouter.ai/api/v1/chat/completions",
        api_key_env: "OPENROUTER_API_KEY",
    },
    ProviderConfig {
        name: "groq",
        display_name: "Groq",
        api_url: "https://api.groq.com/openai/v1/chat/completions",
        api_key_env: "GROQ_API_KEY",
    },
    ProviderConfig {
        name: "cerebras",
        display_name: "Cerebras",
        api_url: "https://api.cerebras.ai/v1/chat/completions",
        api_key_env: "CEREBRAS_API_KEY",
    },
    ProviderConfig {
        name: "zenmux",
        display_name: "Zenmux",
        api_url: "https://zenmux.ai/api/v1/chat/completions",
        api_key_env: "ZENMUX_API_KEY",
    },
    ProviderConfig {
        name: "zai",
        display_name: "Z.AI (Zhipu)",
        api_url: "https://api.z.ai/api/paas/v4/chat/completions",
        api_key_env: "ZAI_API_KEY",
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
// Runtime Model Validation
// ============================================================================

/// Error type for model registry operations.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Failed to parse API response.
    #[error("Failed to parse API response: {0}")]
    ParseError(String),

    /// Provider not found.
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    /// Cache error.
    #[error("Cache error: {0}")]
    CacheError(String),

    /// IO error.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Model validation error - invalid model ID.
    #[error("Invalid model ID: {model_id}")]
    ModelValidation {
        /// The invalid model ID provided by the user.
        model_id: String,
    },
}

/// Cached model information from API responses.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CachedModel {
    /// Model identifier from the provider API.
    pub id: String,
    /// Human-readable model name.
    pub name: Option<String>,
    /// Whether the model is free to use.
    pub is_free: Option<bool>,
    /// Maximum context window size in tokens.
    pub context_window: Option<u32>,
}

/// Trait for runtime model validation and listing.
#[async_trait]
pub trait ModelRegistry: Send + Sync {
    /// List all available models for a provider.
    async fn list_models(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError>;

    /// Check if a model exists for a provider.
    async fn model_exists(&self, provider: &str, model_id: &str) -> Result<bool, RegistryError>;

    /// Validate that a model ID exists for a provider.
    async fn validate_model(&self, provider: &str, model_id: &str) -> Result<(), RegistryError>;
}

/// Cached model registry with HTTP client and TTL support.
pub struct CachedModelRegistry<'a> {
    cache: crate::cache::FileCacheImpl<Vec<CachedModel>>,
    client: reqwest::Client,
    token_provider: &'a dyn TokenProvider,
}

impl CachedModelRegistry<'_> {
    /// Create a new cached model registry.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for storing cached model lists
    /// * `ttl_seconds` - Time-to-live for cache entries (default: 86400 = 24 hours)
    /// * `token_provider` - Token provider for API credentials
    #[must_use]
    pub fn new(
        cache_dir: PathBuf,
        ttl_seconds: u64,
        token_provider: &dyn TokenProvider,
    ) -> CachedModelRegistry<'_> {
        let ttl = chrono::Duration::seconds(ttl_seconds.try_into().unwrap_or(86400));
        CachedModelRegistry {
            cache: crate::cache::FileCacheImpl::with_dir(cache_dir, "models", ttl),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            token_provider,
        }
    }

    /// Parse `OpenRouter` API response into models.
    fn parse_openrouter_models(data: &serde_json::Value) -> Vec<CachedModel> {
        data.get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        Some(CachedModel {
                            id: m.get("id")?.as_str()?.to_string(),
                            name: m.get("name").and_then(|n| n.as_str()).map(String::from),
                            is_free: m
                                .get("pricing")
                                .and_then(|p| p.get("prompt"))
                                .and_then(|p| p.as_str())
                                .map(|p| p == "0"),
                            context_window: m
                                .get("context_length")
                                .and_then(serde_json::Value::as_u64)
                                .and_then(|c| u32::try_from(c).ok()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Parse Gemini API response into models.
    fn parse_gemini_models(data: &serde_json::Value) -> Vec<CachedModel> {
        data.get("models")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        Some(CachedModel {
                            id: m.get("name")?.as_str()?.to_string(),
                            name: m
                                .get("displayName")
                                .and_then(|n| n.as_str())
                                .map(String::from),
                            is_free: None,
                            context_window: m
                                .get("inputTokenLimit")
                                .and_then(serde_json::Value::as_u64)
                                .and_then(|c| u32::try_from(c).ok()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Parse generic OpenAI-compatible API response into models.
    fn parse_generic_models(data: &serde_json::Value) -> Vec<CachedModel> {
        data.get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        Some(CachedModel {
                            id: m.get("id")?.as_str()?.to_string(),
                            name: None,
                            is_free: None,
                            context_window: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Fetch models from provider API.
    async fn fetch_from_api(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError> {
        let url = match provider {
            "openrouter" => "https://openrouter.ai/api/v1/models",
            "gemini" => "https://generativelanguage.googleapis.com/v1beta/models",
            "groq" => "https://api.groq.com/openai/v1/models",
            "cerebras" => "https://api.cerebras.ai/v1/models",
            "zenmux" => "https://zenmux.ai/api/v1/models",
            "zai" => "https://api.z.ai/api/paas/v4/models",
            _ => return Err(RegistryError::ProviderNotFound(provider.to_string())),
        };

        // Get API key from token provider
        let api_key = self.token_provider.ai_api_key(provider).ok_or_else(|| {
            RegistryError::HttpError(format!("No API key available for {provider}"))
        })?;

        // Build request incrementally with provider-specific authentication
        let request = match provider {
            "gemini" => {
                // Gemini uses query parameter authentication
                self.client
                    .get(url)
                    .query(&[("key", api_key.expose_secret())])
            }
            "openrouter" | "groq" | "cerebras" | "zenmux" | "zai" => {
                // These providers use Bearer token authentication
                self.client.get(url).header(
                    "Authorization",
                    format!("Bearer {}", api_key.expose_secret()),
                )
            }
            _ => self.client.get(url),
        };

        let response = request
            .send()
            .await
            .map_err(|e| RegistryError::HttpError(e.to_string()))?;

        let data = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| RegistryError::HttpError(e.to_string()))?;

        // Parse based on provider API format
        let models = match provider {
            "openrouter" => Self::parse_openrouter_models(&data),
            "gemini" => Self::parse_gemini_models(&data),
            "groq" | "cerebras" | "zenmux" | "zai" => Self::parse_generic_models(&data),
            _ => vec![],
        };

        Ok(models)
    }
}

#[async_trait]
impl ModelRegistry for CachedModelRegistry<'_> {
    async fn list_models(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError> {
        // Try fresh cache first
        if let Ok(Some(models)) = self.cache.get(provider) {
            return Ok(models);
        }

        // Fetch from API with stale fallback
        match self.fetch_from_api(provider).await {
            Ok(models) => {
                // Save to cache (ignore errors)
                let _ = self.cache.set(provider, &models);
                Ok(models)
            }
            Err(api_error) => {
                // Try stale cache as fallback
                match self.cache.get_stale(provider) {
                    Ok(Some(models)) => {
                        tracing::warn!(
                            provider = provider,
                            error = %api_error,
                            "API request failed, returning stale cached models"
                        );
                        Ok(models)
                    }
                    _ => {
                        // No stale cache available, return original API error
                        Err(api_error)
                    }
                }
            }
        }
    }

    async fn model_exists(&self, provider: &str, model_id: &str) -> Result<bool, RegistryError> {
        let models = self.list_models(provider).await?;
        Ok(models.iter().any(|m| m.id == model_id))
    }

    async fn validate_model(&self, provider: &str, model_id: &str) -> Result<(), RegistryError> {
        if self.model_exists(provider, model_id).await? {
            Ok(())
        } else {
            Err(RegistryError::ModelValidation {
                model_id: model_id.to_string(),
            })
        }
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
    }

    #[test]
    fn test_get_provider_openrouter() {
        let provider = get_provider("openrouter");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "OpenRouter");
        assert_eq!(provider.api_key_env, "OPENROUTER_API_KEY");
    }

    #[test]
    fn test_get_provider_groq() {
        let provider = get_provider("groq");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Groq");
        assert_eq!(provider.api_key_env, "GROQ_API_KEY");
    }

    #[test]
    fn test_get_provider_cerebras() {
        let provider = get_provider("cerebras");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Cerebras");
        assert_eq!(provider.api_key_env, "CEREBRAS_API_KEY");
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
    fn test_get_provider_zenmux() {
        let provider = get_provider("zenmux");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Zenmux");
        assert_eq!(provider.api_key_env, "ZENMUX_API_KEY");
    }

    #[test]
    fn test_get_provider_zai() {
        let provider = get_provider("zai");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Z.AI (Zhipu)");
        assert_eq!(provider.api_key_env, "ZAI_API_KEY");
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
