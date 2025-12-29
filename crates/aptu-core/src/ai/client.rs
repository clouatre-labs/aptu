// SPDX-License-Identifier: Apache-2.0

//! Generic AI client for all registered providers.
//!
//! Provides a single `AiClient` struct that works with any AI provider
//! registered in the provider registry. See [`super::registry`] for available providers.

use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::SecretString;

use super::circuit_breaker::CircuitBreaker;
use super::provider::AiProvider;
use super::registry::{ProviderConfig, get_provider};
use crate::config::AiConfig;

/// Generic AI client for all providers.
///
/// Holds HTTP client, API key, and model configuration for reuse across multiple requests.
/// Uses the provider registry to get provider-specific configuration.
#[derive(Debug)]
pub struct AiClient {
    /// Provider configuration from registry.
    provider: &'static ProviderConfig,
    /// HTTP client with configured timeout.
    http: Client,
    /// API key for provider authentication.
    api_key: SecretString,
    /// Model name (e.g., "mistralai/devstral-2512:free").
    model: String,
    /// Maximum tokens for API responses.
    max_tokens: u32,
    /// Temperature for API requests.
    temperature: f32,
    /// Circuit breaker for resilience.
    circuit_breaker: CircuitBreaker,
}

impl AiClient {
    /// Creates a new AI client from configuration.
    ///
    /// Validates the model against cost control settings and fetches the API key
    /// from the environment.
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the provider (e.g., "openrouter", "gemini")
    /// * `config` - AI configuration with model, timeout, and cost control settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Provider is not found in registry
    /// - Model is not in free tier and `allow_paid_models` is false (for `OpenRouter`)
    /// - API key environment variable is not set
    /// - HTTP client creation fails
    pub fn new(provider_name: &str, config: &AiConfig) -> Result<Self> {
        // Look up provider in registry
        let provider = get_provider(provider_name)
            .with_context(|| format!("Unknown AI provider: {provider_name}"))?;

        // Validate model against cost control (OpenRouter-specific)
        if provider_name == "openrouter"
            && !config.allow_paid_models
            && !super::is_free_model(&config.model)
        {
            anyhow::bail!(
                "Model '{}' is not in the free tier.\n\
                 To use paid models, set `allow_paid_models = true` in your config file:\n\
                 {}\n\n\
                 Or use a free model like: mistralai/devstral-2512:free",
                config.model,
                crate::config::config_file_path().display()
            );
        }

        // Get API key from environment
        let api_key = env::var(provider.api_key_env).with_context(|| {
            format!(
                "Missing {} environment variable.\n\
                 Set it with: export {}=your_api_key",
                provider.api_key_env, provider.api_key_env
            )
        })?;

        // Create HTTP client with timeout
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            provider,
            http,
            api_key: SecretString::new(api_key.into()),
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            circuit_breaker: CircuitBreaker::new(
                config.circuit_breaker_threshold,
                config.circuit_breaker_reset_seconds,
            ),
        })
    }

    /// Creates a new AI client with a provided API key.
    ///
    /// This constructor allows callers to provide an API key directly,
    /// enabling multi-platform credential resolution (e.g., from iOS keychain via FFI).
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the provider (e.g., "openrouter", "gemini")
    /// * `api_key` - API key as a `SecretString`
    /// * `config` - AI configuration with model, timeout, and cost control settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Provider is not found in registry
    /// - Model is not in free tier and `allow_paid_models` is false (for `OpenRouter`)
    /// - HTTP client creation fails
    pub fn with_api_key(
        provider_name: &str,
        api_key: SecretString,
        config: &AiConfig,
    ) -> Result<Self> {
        // Look up provider in registry
        let provider = get_provider(provider_name)
            .with_context(|| format!("Unknown AI provider: {provider_name}"))?;

        // Validate model against cost control (OpenRouter-specific)
        if provider_name == "openrouter"
            && !config.allow_paid_models
            && !super::is_free_model(&config.model)
        {
            anyhow::bail!(
                "Model '{}' is not in the free tier.\n\
                 To use paid models, set `allow_paid_models = true` in your config file:\n\
                 {}\n\n\
                 Or use a free model like: mistralai/devstral-2512:free",
                config.model,
                crate::config::config_file_path().display()
            );
        }

        // Create HTTP client with timeout
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            provider,
            http,
            api_key,
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            circuit_breaker: CircuitBreaker::new(
                config.circuit_breaker_threshold,
                config.circuit_breaker_reset_seconds,
            ),
        })
    }

    /// Get the circuit breaker for this client.
    #[must_use]
    pub fn circuit_breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }
}

#[async_trait]
impl AiProvider for AiClient {
    fn name(&self) -> &str {
        self.provider.name
    }

    fn api_url(&self) -> &str {
        self.provider.api_url
    }

    fn api_key_env(&self) -> &str {
        self.provider.api_key_env
    }

    fn http_client(&self) -> &Client {
        &self.http
    }

    fn api_key(&self) -> &SecretString {
        &self.api_key
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    fn temperature(&self) -> f32 {
        self.temperature
    }

    fn circuit_breaker(&self) -> Option<&super::CircuitBreaker> {
        Some(&self.circuit_breaker)
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(val) = "application/json".parse() {
            headers.insert("Content-Type", val);
        }

        // OpenRouter-specific headers
        if self.provider.name == "openrouter" {
            if let Ok(val) = "https://github.com/clouatre-labs/aptu".parse() {
                headers.insert("HTTP-Referer", val);
            }
            if let Ok(val) = "Aptu CLI".parse() {
                headers.insert("X-Title", val);
            }
        }

        headers
    }
}

#[cfg(test)]
mod tests {
    use super::super::registry::all_providers;
    use super::*;

    fn test_config() -> AiConfig {
        AiConfig {
            provider: "openrouter".to_string(),
            model: "test-model:free".to_string(),
            max_tokens: 2048,
            temperature: 0.3,
            timeout_seconds: 30,
            allow_paid_models: false,
            circuit_breaker_threshold: 3,
            circuit_breaker_reset_seconds: 60,
        }
    }

    #[test]
    fn test_with_api_key_all_providers() {
        let config = test_config();
        for provider_config in all_providers() {
            let result = AiClient::with_api_key(
                provider_config.name,
                SecretString::from("test_key"),
                &config,
            );
            assert!(
                result.is_ok(),
                "Failed for provider: {}",
                provider_config.name
            );
        }
    }

    #[test]
    fn test_unknown_provider_error() {
        let config = test_config();
        let result = AiClient::with_api_key("nonexistent", SecretString::from("key"), &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_openrouter_rejects_paid_model() {
        let mut config = test_config();
        config.model = "anthropic/claude-3".to_string();
        config.allow_paid_models = false;
        let result = AiClient::with_api_key("openrouter", SecretString::from("key"), &config);
        assert!(result.is_err());
    }
}
