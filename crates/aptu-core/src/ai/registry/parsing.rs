// SPDX-License-Identifier: Apache-2.0

//! HTTP model parsing and caching logic.

use async_trait::async_trait;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

use super::consts::{
    PROVIDER_CEREBRAS, PROVIDER_GEMINI, PROVIDER_GROQ, PROVIDER_OPENROUTER, PROVIDER_ZAI,
    PROVIDER_ZENMUX,
};
use crate::auth::TokenProvider;
use crate::cache::FileCache;

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

/// Model capability indicators.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Model supports image/vision inputs.
    Vision,
    /// Model supports function/tool calling.
    FunctionCalling,
    /// Model has extended reasoning capabilities.
    Reasoning,
}

/// Raw pricing information for a model (cost per token in USD).
///
/// `f64` is used because these values are display-only (never used for
/// arithmetic or financial calculations). Precision matches what the API
/// returns in its JSON responses. If cost estimation or budget tracking is
/// added in the future, migrate to a decimal type such as `rust_decimal`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PricingInfo {
    /// Cost per prompt token in USD. None if unavailable.
    pub prompt_per_token: Option<f64>,
    /// Cost per completion token in USD. None if unavailable.
    pub completion_per_token: Option<f64>,
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
    /// Provider name this model belongs to.
    pub provider: String,
    /// Model capabilities (e.g., `Vision`, `FunctionCalling`).
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// Pricing information for this model.
    #[serde(default)]
    pub pricing: Option<PricingInfo>,
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
#[cfg(not(target_arch = "wasm32"))]
pub struct CachedModelRegistry<'a> {
    cache: crate::cache::FileCacheImpl<Vec<CachedModel>>,
    client: reqwest::Client,
    token_provider: &'a dyn TokenProvider,
}

#[cfg(not(target_arch = "wasm32"))]
impl CachedModelRegistry<'_> {
    /// Create a new cached model registry.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for storing cached model lists (None to disable caching)
    /// * `ttl_seconds` - Time-to-live for cache entries (see `DEFAULT_MODEL_TTL_SECS`)
    /// * `token_provider` - Token provider for API credentials
    #[must_use]
    pub fn new(
        cache_dir: Option<PathBuf>,
        ttl_seconds: u64,
        token_provider: &dyn TokenProvider,
    ) -> CachedModelRegistry<'_> {
        let ttl = chrono::Duration::seconds(
            ttl_seconds
                .try_into()
                .unwrap_or(crate::cache::DEFAULT_MODEL_TTL_SECS.cast_signed()),
        );
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
    fn parse_openrouter_models(data: &serde_json::Value, provider: &str) -> Vec<CachedModel> {
        data.get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let pricing_obj = m.get("pricing");
                        let prompt_per_token = pricing_obj
                            .and_then(|p| p.get("prompt"))
                            .and_then(|p| p.as_str())
                            .and_then(|s| s.parse::<f64>().ok());
                        let completion_per_token = pricing_obj
                            .and_then(|p| p.get("completion"))
                            .and_then(|p| p.as_str())
                            .and_then(|s| s.parse::<f64>().ok());

                        let is_free = match (prompt_per_token, completion_per_token) {
                            (Some(prompt), Some(completion)) => {
                                Some(prompt == 0.0 && completion == 0.0)
                            }
                            (Some(prompt), None) => Some(prompt == 0.0),
                            _ => pricing_obj
                                .and_then(|p| p.get("prompt"))
                                .and_then(|p| p.as_str())
                                .map(|p| p == "0"),
                        };

                        let pricing =
                            if prompt_per_token.is_some() || completion_per_token.is_some() {
                                Some(PricingInfo {
                                    prompt_per_token,
                                    completion_per_token,
                                })
                            } else {
                                None
                            };

                        // Derive capabilities from architecture field defensively
                        let arch = m.get("architecture");
                        let capabilities = {
                            // Check input_modalities array first
                            let from_input_modalities = arch
                                .and_then(|a| a.get("input_modalities"))
                                .and_then(|im| im.as_array())
                                .map(|arr| {
                                    arr.iter().filter_map(|v| v.as_str()).any(|s| s == "image")
                                });
                            // Fall back to modalities string
                            let from_modalities_str = arch
                                .and_then(|a| a.get("modalities"))
                                .and_then(|m| m.as_str())
                                .map(|s| s.contains("image"));

                            let has_vision = from_input_modalities
                                .or(from_modalities_str)
                                .unwrap_or(false);

                            if has_vision {
                                vec![Capability::Vision]
                            } else {
                                vec![]
                            }
                        };

                        Some(CachedModel {
                            id: m.get("id")?.as_str()?.to_string(),
                            name: m.get("name").and_then(|n| n.as_str()).map(String::from),
                            is_free,
                            context_window: m
                                .get("context_length")
                                .and_then(serde_json::Value::as_u64)
                                .and_then(|c| u32::try_from(c).ok()),
                            provider: provider.to_string(),
                            capabilities,
                            pricing,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Parse Gemini API response into models.
    fn parse_gemini_models(data: &serde_json::Value, provider: &str) -> Vec<CachedModel> {
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
                            provider: provider.to_string(),
                            capabilities: vec![],
                            pricing: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Parse generic OpenAI-compatible API response into models.
    fn parse_generic_models(data: &serde_json::Value, provider: &str) -> Vec<CachedModel> {
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
                            provider: provider.to_string(),
                            capabilities: vec![],
                            pricing: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Fetch models from provider API.
    async fn fetch_from_api(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError> {
        let url = match provider {
            PROVIDER_OPENROUTER => "https://openrouter.ai/api/v1/models",
            PROVIDER_GEMINI => "https://generativelanguage.googleapis.com/v1beta/models",
            PROVIDER_GROQ => "https://api.groq.com/openai/v1/models",
            PROVIDER_CEREBRAS => "https://api.cerebras.ai/v1/models",
            PROVIDER_ZENMUX => "https://zenmux.ai/api/v1/models",
            PROVIDER_ZAI => "https://api.z.ai/api/paas/v4/models",
            _ => return Err(RegistryError::ProviderNotFound(provider.to_string())),
        };

        // Get API key from token provider
        let api_key = self.token_provider.ai_api_key(provider).ok_or_else(|| {
            RegistryError::HttpError(format!("No API key available for {provider}"))
        })?;

        // Build request incrementally with provider-specific authentication
        let request = match provider {
            PROVIDER_GEMINI => {
                // Gemini uses header authentication
                self.client
                    .get(url)
                    .header("x-goog-api-key", api_key.expose_secret())
            }
            PROVIDER_OPENROUTER | PROVIDER_GROQ | PROVIDER_CEREBRAS | PROVIDER_ZENMUX
            | PROVIDER_ZAI => {
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
            PROVIDER_OPENROUTER => Self::parse_openrouter_models(&data, provider),
            PROVIDER_GEMINI => Self::parse_gemini_models(&data, provider),
            PROVIDER_GROQ | PROVIDER_CEREBRAS | PROVIDER_ZENMUX | PROVIDER_ZAI => {
                Self::parse_generic_models(&data, provider)
            }
            _ => vec![],
        };

        Ok(models)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl ModelRegistry for CachedModelRegistry<'_> {
    async fn list_models(&self, provider: &str) -> Result<Vec<CachedModel>, RegistryError> {
        // Try fresh cache first
        if let Ok(Some(models)) = self.cache.get(provider).await {
            return Ok(models);
        }

        // Fetch from API with stale fallback
        match self.fetch_from_api(provider).await {
            Ok(models) => {
                // Save to cache (ignore errors)
                let _ = self.cache.set(provider, &models).await;
                Ok(models)
            }
            Err(api_error) => {
                // Try stale cache as fallback
                match self.cache.get_stale(provider).await {
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
    fn test_parse_openrouter_models_with_pricing() {
        let data = serde_json::json!({
            "data": [
                {
                    "id": "openai/gpt-4o",
                    "name": "GPT-4o",
                    "context_length": 128_000,
                    "pricing": {
                        "prompt": "0.000005",
                        "completion": "0.000015"
                    },
                    "architecture": {
                        "input_modalities": ["text", "image"],
                        "output_modalities": ["text"]
                    }
                }
            ]
        });

        let models = CachedModelRegistry::parse_openrouter_models(&data, "openrouter");
        assert_eq!(models.len(), 1);
        let m = &models[0];
        assert_eq!(m.id, "openai/gpt-4o");
        assert_eq!(m.is_free, Some(false));
        let pricing = m.pricing.as_ref().expect("pricing should be present");
        assert_eq!(pricing.prompt_per_token, Some(0.000_005));
        assert_eq!(pricing.completion_per_token, Some(0.000_015));
        assert!(m.capabilities.contains(&Capability::Vision));
    }

    #[test]
    fn test_parse_openrouter_models_missing_capabilities() {
        let data = serde_json::json!({
            "data": [
                {
                    "id": "some/text-only-model",
                    "name": "Text Only",
                    "context_length": 32000,
                    "pricing": {
                        "prompt": "0",
                        "completion": "0"
                    }
                }
            ]
        });

        let models = CachedModelRegistry::parse_openrouter_models(&data, "openrouter");
        assert_eq!(models.len(), 1);
        let m = &models[0];
        assert!(
            m.capabilities.is_empty(),
            "no vision if architecture missing"
        );
        assert_eq!(m.is_free, Some(true));
    }
}
