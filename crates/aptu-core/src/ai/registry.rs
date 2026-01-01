// SPDX-License-Identifier: Apache-2.0

//! Centralized provider configuration registry.
//!
//! This module provides a static registry of all AI providers supported by Aptu,
//! including their metadata, API endpoints, and available models.
//!
//! It also provides runtime model validation infrastructure via the `ModelRegistry` trait
//! and `CachedModelRegistry` implementation for fetching and caching model lists from
//! provider APIs.
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
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::auth::TokenProvider;

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

/// Cache metadata for TTL validation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// Unix timestamp when the cache entry was created.
    pub timestamp: u64,
    /// Time-to-live in seconds.
    pub ttl_seconds: u64,
}

/// Trait for runtime model validation and listing.
#[async_trait]
pub trait ModelRegistry: Send + Sync {
    /// List all available models for a provider.
    async fn list_models(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError>;

    /// Check if a model exists for a provider.
    async fn model_exists(&self, provider: &str, model_id: &str) -> Result<bool, RegistryError>;

    /// Suggest similar models when a model is not found.
    async fn suggest_similar(
        &self,
        provider: &str,
        model_id: &str,
    ) -> Result<Vec<String>, RegistryError>;
}

/// Cached model registry with HTTP client and TTL support.
pub struct CachedModelRegistry<T: TokenProvider + ?Sized> {
    cache_dir: PathBuf,
    ttl_seconds: u64,
    client: reqwest::Client,
    token_provider: Arc<T>,
}

impl<T: TokenProvider + ?Sized> CachedModelRegistry<T> {
    /// Create a new cached model registry.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for storing cached model lists
    /// * `ttl_seconds` - Time-to-live for cache entries (default: 86400 = 24 hours)
    /// * `token_provider` - Token provider for API credentials
    #[must_use]
    pub fn new(cache_dir: PathBuf, ttl_seconds: u64, token_provider: Arc<T>) -> Self {
        Self {
            cache_dir,
            ttl_seconds,
            client: reqwest::Client::new(),
            token_provider,
        }
    }

    /// Get the cache file path for a provider.
    fn cache_path(&self, provider: &str) -> PathBuf {
        self.cache_dir.join(format!("models_{provider}.json"))
    }

    /// Check if cache is still valid.
    fn is_cache_valid(metadata: &CacheMetadata) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now < metadata.timestamp + metadata.ttl_seconds
    }

    /// Load models from cache, respecting TTL.
    fn load_from_cache(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError> {
        let path = self.cache_path(provider);
        if !path.exists() {
            return Err(RegistryError::CacheError(
                "Cache file not found".to_string(),
            ));
        }

        let content = std::fs::read_to_string(&path)?;
        let data: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| RegistryError::ParseError(e.to_string()))?;

        // Check TTL and extract models in one step
        if let Some(metadata) = data
            .get("metadata")
            .and_then(|m| serde_json::from_value::<CacheMetadata>(m.clone()).ok())
        {
            if Self::is_cache_valid(&metadata) {
                return data
                    .get("models")
                    .and_then(|m| serde_json::from_value::<Vec<CachedModel>>(m.clone()).ok())
                    .ok_or_else(|| RegistryError::ParseError("Invalid cache format".to_string()));
            }
            return Err(RegistryError::CacheError("Cache expired".to_string()));
        }

        // Extract models if no metadata
        data.get("models")
            .and_then(|m| serde_json::from_value::<Vec<CachedModel>>(m.clone()).ok())
            .ok_or_else(|| RegistryError::ParseError("Invalid cache format".to_string()))
    }

    /// Load models from cache regardless of TTL (stale fallback).
    fn load_stale_cache(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError> {
        let path = self.cache_path(provider);
        if !path.exists() {
            return Err(RegistryError::CacheError(
                "Cache file not found".to_string(),
            ));
        }

        let content = std::fs::read_to_string(&path)?;
        let data: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| RegistryError::ParseError(e.to_string()))?;

        // Extract models regardless of TTL
        data.get("models")
            .and_then(|m| serde_json::from_value::<Vec<CachedModel>>(m.clone()).ok())
            .ok_or_else(|| RegistryError::ParseError("Invalid cache format".to_string()))
    }

    /// Save models to cache.
    fn save_to_cache(&self, provider: &str, models: &[CachedModel]) -> Result<(), RegistryError> {
        std::fs::create_dir_all(&self.cache_dir)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cache_data = serde_json::json!({
            "metadata": {
                "timestamp": now,
                "ttl_seconds": self.ttl_seconds,
            },
            "models": models,
        });

        let path = self.cache_path(provider);
        std::fs::write(&path, cache_data.to_string())?;
        Ok(())
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

        let mut request = self.client.get(url);

        // Add authentication based on provider
        match provider {
            "gemini" => {
                // Gemini uses query parameter authentication
                request = request.query(&[("key", api_key.expose_secret())]);
            }
            "openrouter" | "groq" | "cerebras" | "zenmux" | "zai" => {
                // These providers use Bearer token authentication
                request = request.header(
                    "Authorization",
                    format!("Bearer {}", api_key.expose_secret()),
                );
            }
            _ => {}
        }

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
            "openrouter" => data
                .get("data")
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
                .unwrap_or_default(),
            "gemini" => data
                .get("models")
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
                .unwrap_or_default(),
            "groq" | "cerebras" => data
                .get("data")
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
                .unwrap_or_default(),
            _ => vec![],
        };

        Ok(models)
    }
}

#[async_trait]
impl<T: TokenProvider + ?Sized> ModelRegistry for CachedModelRegistry<T> {
    async fn list_models(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError> {
        // Try fresh cache first
        if let Ok(models) = self.load_from_cache(provider) {
            return Ok(models);
        }

        // Fetch from API with stale fallback
        match self.fetch_from_api(provider).await {
            Ok(models) => {
                // Save to cache (ignore errors)
                let _ = self.save_to_cache(provider, &models);
                Ok(models)
            }
            Err(api_error) => {
                // Try stale cache as fallback
                match self.load_stale_cache(provider) {
                    Ok(models) => {
                        tracing::warn!(
                            provider = provider,
                            error = %api_error,
                            "API request failed, returning stale cached models"
                        );
                        Ok(models)
                    }
                    Err(_) => {
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

    async fn suggest_similar(
        &self,
        provider: &str,
        model_id: &str,
    ) -> Result<Vec<String>, RegistryError> {
        let models = self.list_models(provider).await?;
        let last_part = model_id.split('/').next_back().unwrap_or(model_id);
        let mut suggestions: Vec<_> = models
            .iter()
            .filter(|m| m.id.contains(last_part) || model_id.contains(&m.id))
            .map(|m| m.id.clone())
            .collect();
        suggestions.truncate(5);
        Ok(suggestions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_stale_cache_ignores_ttl() {
        // Arrange: Create a temporary cache file with expired TTL
        let temp_dir = std::env::temp_dir().join("aptu_test_stale_cache");
        let _ = std::fs::create_dir_all(&temp_dir);

        let registry = CachedModelRegistry::new(temp_dir.clone(), 1); // 1 second TTL

        let models = vec![
            CachedModel {
                id: "test-model-1".to_string(),
                name: Some("Test Model 1".to_string()),
                is_free: Some(true),
                context_window: Some(4096),
            },
            CachedModel {
                id: "test-model-2".to_string(),
                name: Some("Test Model 2".to_string()),
                is_free: Some(false),
                context_window: Some(8192),
            },
        ];

        // Save to cache
        let _ = registry.save_to_cache("test_provider", &models);

        // Wait for TTL to expire
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Act: Load stale cache (should succeed despite expired TTL)
        let result = registry.load_stale_cache("test_provider");

        // Assert
        assert!(result.is_ok(), "load_stale_cache should succeed");
        let loaded_models = result.unwrap();
        assert_eq!(loaded_models.len(), 2);
        assert_eq!(loaded_models[0].id, "test-model-1");
        assert_eq!(loaded_models[1].id, "test-model-2");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

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
