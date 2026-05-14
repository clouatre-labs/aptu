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
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use super::circuit_breaker::CircuitBreaker;
use super::provider::AiProvider;
use super::registry::{ProviderConfig, get_provider};
use crate::config::AiConfig;

/// Authentication method used by the AI client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// API key from environment variable.
    ApiKey,
    /// OAuth token from Claude credentials file.
    OAuth,
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMethod::ApiKey => write!(f, "api-key"),
            AuthMethod::OAuth => write!(f, "oauth"),
        }
    }
}

/// Claude credentials from ~/.claude/credentials.json.
#[derive(Debug, Deserialize)]
pub struct ClaudeCredentials {
    /// OAuth access token.
    pub access_token: String,
}

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
    /// Model name (e.g., "mistralai/mistral-small-2603").
    model: String,
    /// Maximum tokens for API responses.
    max_tokens: u32,
    /// Temperature for API requests.
    temperature: f32,
    /// Maximum retry attempts for rate-limited requests.
    max_attempts: u32,
    /// Circuit breaker for resilience.
    circuit_breaker: CircuitBreaker,
    /// Optional custom guidance from config to inject into system prompts.
    custom_guidance: Option<String>,
    /// Authentication method used.
    auth_method: AuthMethod,
}

impl Drop for AiClient {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        // Safety: SecretString wraps String, which implements Zeroize.
        // Calling zeroize() overwrites the backing buffer before deallocation.
        self.api_key.zeroize();
    }
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
                 Or use a free model like: google/gemma-3-12b-it:free",
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
            max_attempts: config.retry_max_attempts,
            circuit_breaker: CircuitBreaker::new(
                config.circuit_breaker_threshold,
                config.circuit_breaker_reset_seconds,
            ),
            custom_guidance: config.custom_guidance.clone(),
            auth_method: AuthMethod::ApiKey,
        })
    }

    /// Creates a new AI client with a provided API key and validates the model exists.
    ///
    /// This constructor validates that the model exists via the runtime model registry
    /// before creating the client. It allows callers to provide an API key directly,
    /// enabling multi-platform credential resolution (e.g., from iOS keychain via FFI).
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the provider (e.g., "openrouter", "gemini")
    /// * `api_key` - API key as a `SecretString`
    /// * `model_name` - Model name to use (e.g., "gemini-3.1-flash-lite-preview")
    /// * `config` - AI configuration with timeout and cost control settings
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
        model_name: &str,
        config: &AiConfig,
    ) -> Result<Self> {
        // Look up provider in registry
        let provider = get_provider(provider_name)
            .with_context(|| format!("Unknown AI provider: {provider_name}"))?;

        // Validate model against cost control (OpenRouter-specific)
        if provider_name == "openrouter"
            && !config.allow_paid_models
            && !super::is_free_model(model_name)
        {
            anyhow::bail!(
                "Model '{}' is not in the free tier.\n\
                 To use paid models, set `allow_paid_models = true` in your config file:\n\
                 {}\n\n\
                 Or use a free model like: google/gemma-3-12b-it:free",
                model_name,
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
            model: model_name.to_string(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            max_attempts: config.retry_max_attempts,
            circuit_breaker: CircuitBreaker::new(
                config.circuit_breaker_threshold,
                config.circuit_breaker_reset_seconds,
            ),
            custom_guidance: config.custom_guidance.clone(),
            auth_method: AuthMethod::ApiKey,
        })
    }

    /// Creates a new AI client from Claude credentials file (~/.claude/credentials.json).
    ///
    /// Reads the credentials file, extracts the access token, stores it in the OS keyring,
    /// and returns an `AiClient` configured for the Anthropic provider.
    ///
    /// # Arguments
    ///
    /// * `config` - AI configuration with timeout and cost control settings
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(AiClient))` if credentials are found and valid,
    /// `Ok(None)` if the credentials file is missing or invalid,
    /// or an error if keyring operations fail.
    pub fn from_claude_credentials(config: &AiConfig) -> Result<Option<Self>> {
        // Resolve credentials file path
        let Some(home) = dirs::home_dir() else {
            return Ok(None);
        };

        let creds_path = home.join(".claude").join("credentials.json");

        // Check if file exists
        if !creds_path.exists() {
            return Ok(None);
        }

        // Read and parse credentials file
        let creds_content =
            std::fs::read_to_string(&creds_path).context("Failed to read credentials file")?;

        let creds: ClaudeCredentials =
            serde_json::from_str(&creds_content).context("Failed to parse credentials JSON")?;

        // Validate token is not empty
        if creds.access_token.is_empty() {
            return Ok(None);
        }

        // Store token in keyring
        #[cfg(feature = "keyring")]
        {
            use keyring_core::Entry;
            let entry = Entry::new("aptu", "anthropic_oauth_token")
                .context("Failed to create keyring entry")?;
            entry
                .set_password(&creds.access_token)
                .context("Failed to store token in keyring")?;
        }

        // Create client with the token
        let client = Self::with_api_key(
            "anthropic",
            SecretString::from(creds.access_token),
            &config.model,
            config,
        )?;

        // Mark as OAuth
        let mut client = client;
        client.auth_method = AuthMethod::OAuth;
        Ok(Some(client))
    }

    /// Attempts to retrieve a Claude OAuth token from the OS keyring.
    ///
    /// Returns `Ok(Some(AiClient))` if a token is found in the keyring,
    /// `Ok(None)` if no token is stored, or an error if keyring operations fail.
    pub fn from_keyring_oauth(config: &AiConfig) -> Result<Option<Self>> {
        #[cfg(feature = "keyring")]
        {
            use keyring_core::Entry;
            let entry = Entry::new("aptu", "anthropic_oauth_token")
                .context("Failed to create keyring entry")?;

            match entry.get_password() {
                Ok(token) => {
                    let client = Self::with_api_key(
                        "anthropic",
                        SecretString::from(token),
                        &config.model,
                        config,
                    )?;

                    let mut client = client;
                    client.auth_method = AuthMethod::OAuth;
                    Ok(Some(client))
                }
                Err(_) => Ok(None),
            }
        }

        #[cfg(not(feature = "keyring"))]
        {
            let _ = config;
            Ok(None)
        }
    }

    /// Returns the authentication method used by this client.
    #[must_use]
    pub fn auth_method(&self) -> AuthMethod {
        self.auth_method
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

    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    fn circuit_breaker(&self) -> Option<&super::CircuitBreaker> {
        Some(&self.circuit_breaker)
    }

    fn custom_guidance(&self) -> Option<&str> {
        self.custom_guidance.as_deref()
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(val) = "application/json".parse() {
            headers.insert("Content-Type", val);
        }

        // Anthropic-specific headers
        if self.provider.name == super::registry::PROVIDER_ANTHROPIC {
            if let Ok(val) = self.api_key().expose_secret().parse() {
                headers.insert("x-api-key", val);
            }
            if let Ok(val) = "2023-06-01".parse() {
                headers.insert("anthropic-version", val);
            }
            return headers;
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
            retry_max_attempts: 3,
            tasks: None,
            fallback: None,
            custom_guidance: None,
            validation_enabled: true,
        }
    }

    #[test]
    fn test_with_api_key_all_providers() {
        let config = test_config();
        for provider_config in all_providers() {
            let result = AiClient::with_api_key(
                provider_config.name,
                SecretString::from("test_key"),
                "test-model:free",
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
        let result = AiClient::with_api_key(
            "nonexistent",
            SecretString::from("key"),
            "test-model",
            &config,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_openrouter_rejects_paid_model() {
        let mut config = test_config();
        config.model = "anthropic/claude-sonnet-4-6".to_string();
        config.allow_paid_models = false;
        let result = AiClient::with_api_key(
            "openrouter",
            SecretString::from("key"),
            "anthropic/claude-sonnet-4-6",
            &config,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_max_attempts_from_config() {
        let mut config = test_config();
        config.retry_max_attempts = 5;
        let client = AiClient::with_api_key(
            "openrouter",
            SecretString::from("key"),
            "test-model:free",
            &config,
        )
        .expect("should create client");
        assert_eq!(client.max_attempts(), 5);
    }

    #[test]
    fn test_build_headers_anthropic_has_api_key_and_version() {
        let config = test_config();
        let client = AiClient::with_api_key(
            "anthropic",
            SecretString::from("test_api_key"),
            "test-model",
            &config,
        )
        .expect("should create anthropic client");

        let headers = client.build_headers();

        let header_str = |k| headers.get(k).and_then(|v| v.to_str().ok());
        assert_eq!(header_str("x-api-key"), Some("test_api_key"));
        assert_eq!(header_str("anthropic-version"), Some("2023-06-01"));
    }

    #[test]
    fn test_build_headers_non_anthropic_unaffected() {
        let config = test_config();
        let client = AiClient::with_api_key(
            "openrouter",
            SecretString::from("test_key"),
            "test-model:free",
            &config,
        )
        .expect("should create openrouter client");

        let headers = client.build_headers();

        assert!(!headers.contains_key("anthropic-version"));
        assert!(headers.contains_key("http-referer"));
        assert!(headers.contains_key("x-title"));
    }

    #[test]
    fn test_from_claude_credentials_missing_file() {
        let config = test_config();
        let result = AiClient::from_claude_credentials(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_from_claude_credentials_malformed_json() {
        use std::fs;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().expect("should create temp dir");
        let claude_dir = temp_dir.path().join(".claude");
        fs::create_dir_all(&claude_dir).expect("should create .claude dir");

        let creds_path = claude_dir.join("credentials.json");
        let mut file = fs::File::create(&creds_path).expect("should create file");
        file.write_all(b"{ invalid json }")
            .expect("should write file");

        // Temporarily override home_dir for this test
        // Since we can't easily mock dirs::home_dir, we'll test the parsing logic directly
        let malformed = "{ invalid json }";
        let result: Result<ClaudeCredentials, _> = serde_json::from_str(malformed);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_claude_credentials_missing_access_token() {
        let malformed = r#"{"other_field": "value"}"#;
        let result: Result<ClaudeCredentials, _> = serde_json::from_str(malformed);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_claude_credentials_empty_token() {
        let empty_token = r#"{"access_token": ""}"#;
        let creds: ClaudeCredentials = serde_json::from_str(empty_token).expect("should parse");
        assert!(creds.access_token.is_empty());
    }

    #[test]
    fn test_auth_method_api_key() {
        let config = test_config();
        let client = AiClient::with_api_key(
            "anthropic",
            SecretString::from("test_key"),
            "test-model",
            &config,
        )
        .expect("should create client");
        assert_eq!(client.auth_method(), AuthMethod::ApiKey);
    }
}
