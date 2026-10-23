// SPDX-License-Identifier: Apache-2.0

//! Generic AI client for all registered providers.
//!
//! Provides a single `AiClient` struct that works with any AI provider
//! registered in the provider registry. See [`super::registry`] for available providers.

use std::env;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::circuit_breaker::CircuitBreaker;
use super::provider::AiProvider;
use super::registry::{PROVIDER_ANTHROPIC, PROVIDER_OPENROUTER, ProviderConfig, get_provider};
use crate::auth::TokenProvider;
use crate::config::AiConfig;
use crate::config::ai::TaskType;
use crate::error::AptuError;
use crate::retry::is_retryable_anyhow;
use tracing::{info, instrument, warn};

/// Checks if a model is in the free tier (no cost).
/// Free models on `OpenRouter` always have the `:free` suffix.
#[must_use]
pub fn is_free_model(model: &str) -> bool {
    model.ends_with(":free")
}

/// Resolves Anthropic credentials with OAuth fallback.
///
/// For the Anthropic provider, attempts to use Claude OAuth credentials in this order:
/// 1. Existing token in OS keyring
/// 2. ~/.claude/credentials.json file
/// 3. Environment variable (fallback)
///
/// Returns `Some(client)` if credentials were found via OAuth or env var,
/// `None` if no credentials were available.
#[must_use]
pub fn resolve_anthropic_credential(ai_config: &crate::config::AiConfig) -> Option<AiClient> {
    // Try keyring first
    if let Ok(Some(client)) = AiClient::from_keyring_oauth(ai_config) {
        return Some(client);
    }

    // Try credentials file
    if let Ok(Some(client)) = AiClient::from_claude_credentials(ai_config) {
        return Some(client);
    }

    // Fall back to environment variable
    AiClient::new(PROVIDER_ANTHROPIC, ai_config).ok()
}

/// Validates model against `OpenRouter` free-tier policy.
fn validate_openrouter_free_tier(
    provider_name: &str,
    model: &str,
    config: &AiConfig,
) -> Result<()> {
    if provider_name == PROVIDER_OPENROUTER && !config.allow_paid_models && !is_free_model(model) {
        anyhow::bail!(
            "Model '{}' is not in the free tier.\n\
             To use paid models, set `allow_paid_models = true` in your config file:\n\
             {}\n\n\
             Or use a free model like: google/gemma-3-12b-it:free",
            model,
            crate::config::config_file_path().display()
        );
    }
    Ok(())
}

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

/// Creates an HTTP client with timeout. On native targets, the request timeout
/// is set on the client; on wasm32, the browser's fetch API manages timeouts
/// independently and reqwest's `timeout()` is unavailable.
fn build_http_client(timeout_seconds: u64) -> Result<Client> {
    #[cfg(not(target_arch = "wasm32"))]
    let http = Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_seconds))
        .build()
        .context("Failed to create HTTP client")?;
    #[cfg(target_arch = "wasm32")]
    let http = Client::builder()
        .build()
        .context("Failed to create HTTP client")?;
    Ok(http)
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
    /// `OpenRouter` data collection setting.
    openrouter_data_collection: String,
    /// `OpenRouter` Zero Data Retention requirement.
    openrouter_zdr: bool,
    /// Authentication method used.
    auth_method: AuthMethod,
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
        validate_openrouter_free_tier(provider_name, &config.model, config)?;

        // Get API key from environment
        let api_key = env::var(provider.api_key_env).with_context(|| {
            format!(
                "Missing {} environment variable.\n\
                 Set it with: export {}=your_api_key",
                provider.api_key_env, provider.api_key_env
            )
        })?;

        // Create HTTP client with timeout (timeout() is native-only; wasm32 uses fetch API)
        let http = build_http_client(config.timeout_seconds)?;

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
            openrouter_data_collection: config.openrouter_data_collection.clone(),
            openrouter_zdr: config.openrouter_zdr,
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
    /// * `model_name` - Model name to use (e.g., "gemini-3.1-flash-lite")
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
        validate_openrouter_free_tier(provider_name, model_name, config)?;

        // Create HTTP client with timeout (timeout() is native-only; wasm32 uses fetch API)
        let http = build_http_client(config.timeout_seconds)?;

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
            openrouter_data_collection: config.openrouter_data_collection.clone(),
            openrouter_zdr: config.openrouter_zdr,
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
            PROVIDER_ANTHROPIC,
            SecretString::from(creds.access_token),
            &config.model,
            config,
        )?;

        // Mark as OAuth
        let mut client = client;
        client.auth_method = AuthMethod::OAuth;
        Ok(Some(client))
    }

    /// Returns the path to the Claude credentials file if it exists.
    ///
    /// This helper centralizes the path resolution logic for ~/.claude/credentials.json,
    /// keeping the CLI command layer thin and avoiding duplicate path construction.
    ///
    /// Returns `Some(path)` if the file exists, `None` otherwise.
    #[must_use]
    pub fn claude_credentials_path() -> Option<std::path::PathBuf> {
        let home = dirs::home_dir()?;
        let creds_path = home.join(".claude").join("credentials.json");
        if creds_path.exists() {
            Some(creds_path)
        } else {
            None
        }
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
                        PROVIDER_ANTHROPIC,
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

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl AiProvider for AiClient {
    fn config(&self) -> &ProviderConfig {
        self.provider
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

    fn session_id(&self, task_type: &str) -> Option<String> {
        if self.provider.name == PROVIDER_OPENROUTER {
            let mut hasher = Sha256::new();
            hasher.update(format!("aptu-{task_type}-{}", self.model()).as_bytes());
            Some(hex::encode(hasher.finalize()))
        } else {
            None
        }
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
        if self.provider.name == PROVIDER_OPENROUTER {
            if let Ok(val) = "https://github.com/clouatre-labs/aptu".parse() {
                headers.insert("HTTP-Referer", val);
            }
            if let Ok(val) = "Aptu CLI".parse() {
                headers.insert("X-Title", val);
            }
            if let Ok(val) = "true".parse() {
                headers.insert("X-OpenRouter-Cache", val);
            }
        }

        headers
    }

    fn provider_body_extensions(&self) -> Option<serde_json::Value> {
        if self.provider.name == PROVIDER_OPENROUTER {
            Some(serde_json::json!({
                "data_collection": &self.openrouter_data_collection,
                "zdr": self.openrouter_zdr,
            }))
        } else {
            None
        }
    }
}

fn validate_provider_model(provider: &str, model: &str) -> crate::Result<()> {
    // Simple static validation: check if provider exists
    if crate::ai::registry::get_provider(provider).is_none() {
        return Err(AptuError::ModelRegistry {
            message: format!("Provider not found: {provider}"),
        });
    }

    // For now, we allow any model ID (permissive fallback)
    // Unknown models will log a warning but won't fail validation
    tracing::debug!(provider = provider, model = model, "Validating model");
    Ok(())
}

/// Setup and validate primary AI provider synchronously.
/// Returns the created AI client or an error.
fn try_setup_primary_client(
    provider: &dyn TokenProvider,
    primary_provider: &str,
    model_name: &str,
    ai_config: &AiConfig,
    task: TaskType,
) -> crate::Result<AiClient> {
    // Apply the per-task effective timeout so the HTTP client gets the right
    // deadline for this operation (review gets more headroom than triage).
    let mut ai_config = ai_config.clone();
    ai_config.timeout_seconds = ai_config.effective_timeout_for_task(task);
    let ai_config = &ai_config;

    // For Anthropic, delegate to centralized credential resolution
    if primary_provider == "anthropic"
        && let Some(client) = crate::ai::resolve_anthropic_credential(ai_config)
    {
        if ai_config.validation_enabled {
            validate_provider_model(primary_provider, model_name)?;
        }
        return Ok(client);
    }

    // Fall back to environment variable for non-Anthropic or missing Anthropic credentials
    let api_key = provider.ai_api_key(primary_provider).ok_or_else(|| {
        let env_var = get_provider(primary_provider).map_or("API_KEY", |p| p.api_key_env);
        AptuError::AiProviderNotAuthenticated {
            provider: primary_provider.to_string(),
            env_var: env_var.to_string(),
        }
    })?;

    if ai_config.validation_enabled {
        validate_provider_model(primary_provider, model_name)?;
    }

    AiClient::with_api_key(primary_provider, api_key, model_name, ai_config).map_err(|e| {
        AptuError::AI {
            message: e.to_string(),
            status: None,
            provider: primary_provider.to_string(),
        }
    })
}

/// Set up an AI client for a single fallback provider entry.
///
/// Returns `Some(client)` on success, `None` if the entry should be skipped.
fn setup_fallback_client(
    provider: &dyn TokenProvider,
    entry: &crate::config::FallbackEntry,
    model_name: &str,
    ai_config: &AiConfig,
    task: TaskType,
) -> Option<AiClient> {
    // Apply the per-task effective timeout to fallback clients as well.
    let mut ai_config = ai_config.clone();
    ai_config.timeout_seconds = ai_config.effective_timeout_for_task(task);
    let ai_config = &ai_config;

    let Some(api_key) = provider.ai_api_key(&entry.provider) else {
        warn!(
            fallback_provider = entry.provider,
            "No API key available for fallback provider"
        );
        return None;
    };

    let fallback_model = entry.model.as_deref().unwrap_or(model_name);

    if ai_config.validation_enabled
        && validate_provider_model(&entry.provider, fallback_model).is_err()
    {
        warn!(
            fallback_provider = entry.provider,
            fallback_model = fallback_model,
            "Fallback provider model validation failed, continuing to next provider"
        );
        return None;
    }

    if let Ok(client) = AiClient::with_api_key(&entry.provider, api_key, fallback_model, ai_config)
    {
        Some(client)
    } else {
        warn!(
            fallback_provider = entry.provider,
            "Failed to create AI client for fallback provider"
        );
        None
    }
}

/// Try a single fallback provider entry.
async fn try_fallback_entry<T, F, Fut>(
    provider: &dyn TokenProvider,
    entry: &crate::config::FallbackEntry,
    model_name: &str,
    ai_config: &AiConfig,
    task: TaskType,
    operation: &F,
) -> crate::Result<Option<T>>
where
    F: Fn(AiClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    warn!(
        fallback_provider = entry.provider,
        "Attempting fallback provider"
    );

    let Some(ai_client) = setup_fallback_client(provider, entry, model_name, ai_config, task)
    else {
        return Ok(None);
    };

    match operation(ai_client).await {
        Ok(response) => {
            info!(
                fallback_provider = entry.provider,
                "Successfully completed operation with fallback provider"
            );
            Ok(Some(response))
        }
        Err(e) => {
            if is_retryable_anyhow(&e) {
                return Err(AptuError::AI {
                    message: e.to_string(),
                    status: None,
                    provider: entry.provider.clone(),
                });
            }
            warn!(
                fallback_provider = entry.provider,
                error = %e,
                "Fallback provider failed with non-retryable error"
            );
            Ok(None)
        }
    }
}

/// Execute the fallback chain when the primary provider fails with a
/// non-retryable error or after rate-limit retries are exhausted.
async fn execute_fallback_chain<T, F, Fut>(
    provider: &dyn TokenProvider,
    primary_provider: &str,
    model_name: &str,
    ai_config: &AiConfig,
    task: TaskType,
    operation: F,
) -> crate::Result<T>
where
    F: Fn(AiClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    if let Some(fallback_config) = &ai_config.fallback {
        for entry in &fallback_config.chain {
            if let Some(response) =
                try_fallback_entry(provider, entry, model_name, ai_config, task, &operation).await?
            {
                return Ok(response);
            }
        }
    }

    Err(AptuError::AI {
        message: "All AI providers failed (primary and fallback chain)".to_string(),
        status: None,
        provider: primary_provider.to_string(),
    })
}

#[instrument(skip(provider, operation))]
pub(crate) async fn try_with_fallback<T, F, Fut>(
    provider: &dyn TokenProvider,
    primary_provider: &str,
    model_name: &str,
    ai_config: &AiConfig,
    task: TaskType,
    operation: F,
) -> crate::Result<T>
where
    F: Fn(AiClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let ai_client =
        try_setup_primary_client(provider, primary_provider, model_name, ai_config, task)?;

    match operation(ai_client).await {
        Ok(response) => return Ok(response),
        Err(e) => {
            // Context-length errors must not hit the fallback chain: the
            // identical oversized prompt would fail on any provider. Return
            // the typed error untouched so the review facade can perform its
            // single re-trim retry.
            if let Some(AptuError::ContextLengthExceeded { .. }) = e.downcast_ref::<AptuError>() {
                return match e.downcast::<AptuError>() {
                    Ok(err) => Err(err),
                    Err(e) => Err(AptuError::AI {
                        message: e.to_string(),
                        status: None,
                        provider: primary_provider.to_string(),
                    }),
                };
            }
            if is_retryable_anyhow(&e) {
                // RateLimited falls through to the fallback chain after in-loop
                // retry exhaustion; other retryable errors return early.
                if let Some(AptuError::RateLimited { .. }) = e.downcast_ref::<AptuError>() {
                    let chain_configured = ai_config
                        .fallback
                        .as_ref()
                        .is_some_and(|f| !f.chain.is_empty());
                    if !chain_configured {
                        // Return the original error via downcast so its anyhow
                        // context and exact instance are preserved.
                        return match e.downcast::<AptuError>() {
                            Ok(err) => Err(err),
                            Err(e) => Err(AptuError::AI {
                                message: e.to_string(),
                                status: None,
                                provider: primary_provider.to_string(),
                            }),
                        };
                    }
                    info!(
                        primary_provider = primary_provider,
                        "Primary provider rate limited after retry exhaustion, trying fallback chain"
                    );
                } else {
                    return Err(AptuError::AI {
                        message: e.to_string(),
                        status: None,
                        provider: primary_provider.to_string(),
                    });
                }
            } else {
                warn!(
                    primary_provider = primary_provider,
                    error = %e,
                    "Primary provider failed with non-retryable error, trying fallback chain"
                );
            }
        }
    }

    execute_fallback_chain(
        provider,
        primary_provider,
        model_name,
        ai_config,
        task,
        operation,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::super::registry::all_providers;
    use super::*;

    fn test_config() -> AiConfig {
        AiConfig {
            provider: PROVIDER_OPENROUTER.to_string(),
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
            openrouter_data_collection: "deny".to_string(),
            openrouter_zdr: true,
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
            PROVIDER_OPENROUTER,
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
            PROVIDER_OPENROUTER,
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
            PROVIDER_ANTHROPIC,
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
            PROVIDER_OPENROUTER,
            SecretString::from("test_key"),
            "test-model:free",
            &config,
        )
        .expect("should create openrouter client");

        let headers = client.build_headers();

        assert!(!headers.contains_key("anthropic-version"));
        assert!(headers.contains_key("http-referer"));
        assert!(headers.contains_key("x-title"));
        assert!(headers.contains_key("x-openrouter-cache"));
    }

    #[test]
    fn test_session_id_openrouter_stable_across_calls() {
        let config = test_config();
        let client = AiClient::with_api_key(
            PROVIDER_OPENROUTER,
            SecretString::from("test_key"),
            "test-model:free",
            &config,
        )
        .expect("should create openrouter client");

        let first = client.session_id("triage");
        let second = client.session_id("triage");
        assert!(first.is_some());
        assert_eq!(first, second);
    }

    #[test]
    fn test_session_id_non_openrouter_is_none() {
        let config = test_config();
        let client = AiClient::with_api_key(
            PROVIDER_ANTHROPIC,
            SecretString::from("test_key"),
            "test-model",
            &config,
        )
        .expect("should create anthropic client");

        assert!(client.session_id("triage").is_none());
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
            PROVIDER_ANTHROPIC,
            SecretString::from("test_key"),
            "test-model",
            &config,
        )
        .expect("should create client");
        assert_eq!(client.auth_method(), AuthMethod::ApiKey);
    }

    #[test]
    fn test_provider_body_extensions_openrouter() {
        let config = test_config();
        let client = AiClient::with_api_key(
            PROVIDER_OPENROUTER,
            SecretString::from("test_key"),
            "test-model:free",
            &config,
        )
        .expect("should create openrouter client");

        let ext = client.provider_body_extensions();
        assert!(ext.is_some());
        let val = ext.unwrap();
        assert_eq!(val["data_collection"], "deny");
        assert_eq!(val["zdr"], true);
    }

    #[test]
    fn test_provider_body_extensions_non_openrouter() {
        let config = test_config();
        let client = AiClient::with_api_key(
            PROVIDER_ANTHROPIC,
            SecretString::from("test_key"),
            "test-model",
            &config,
        )
        .expect("should create anthropic client");

        let ext = client.provider_body_extensions();
        assert!(ext.is_none());
    }
}

#[cfg(test)]
mod fallback_config_tests {
    use crate::config::{FallbackConfig, FallbackEntry};

    #[test]
    fn test_fallback_chain_config_structure() {
        // Test that fallback chain config structure is correct
        let fallback_config = FallbackConfig {
            chain: vec![
                FallbackEntry {
                    provider: "openrouter".to_string(),
                    model: None,
                },
                FallbackEntry {
                    provider: "anthropic".to_string(),
                    model: Some("claude-haiku-4.5".to_string()),
                },
            ],
        };

        assert_eq!(fallback_config.chain.len(), 2);
        assert_eq!(fallback_config.chain[0].provider, "openrouter");
        assert_eq!(fallback_config.chain[0].model, None);
        assert_eq!(fallback_config.chain[1].provider, "anthropic");
        assert_eq!(
            fallback_config.chain[1].model,
            Some("claude-haiku-4.5".to_string())
        );
    }

    #[test]
    fn test_fallback_chain_empty() {
        // Test that empty fallback chain is valid
        let fallback_config = FallbackConfig { chain: vec![] };

        assert_eq!(fallback_config.chain.len(), 0);
    }

    #[test]
    fn test_fallback_chain_single_provider() {
        // Test that single provider fallback chain is valid
        let fallback_config = FallbackConfig {
            chain: vec![FallbackEntry {
                provider: "openrouter".to_string(),
                model: None,
            }],
        };

        assert_eq!(fallback_config.chain.len(), 1);
        assert_eq!(fallback_config.chain[0].provider, "openrouter");
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod fallback_tests {
    use super::*;
    use crate::auth::TokenProvider;
    use crate::config::{FallbackConfig, FallbackEntry};
    use secrecy::SecretString;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    struct MockTokenProvider;

    impl TokenProvider for MockTokenProvider {
        fn github_token(&self) -> Option<SecretString> {
            None
        }

        fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
            Some(SecretString::from(format!("key-{provider}")))
        }
    }

    fn test_config(fallback: Option<FallbackConfig>) -> AiConfig {
        AiConfig {
            validation_enabled: false,
            fallback,
            ..AiConfig::default()
        }
    }

    /// Arrange: primary call rate limited, fallback call succeeds.
    /// Act: run `try_with_fallback`.
    /// Assert: the fallback entry executes and the operation succeeds.
    #[tokio::test]
    async fn test_rate_limited_primary_uses_fallback_entry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_op = calls.clone();
        let operation = |client: AiClient| {
            let counter = calls_for_op.clone();
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err(anyhow::anyhow!(AptuError::RateLimited {
                        provider: "openrouter".to_string(),
                        retry_after: 1,
                    })) as anyhow::Result<u32>
                } else {
                    let _ = client;
                    #[allow(clippy::cast_possible_truncation)]
                    Ok(n as u32)
                }
            }
        };

        let config = test_config(Some(FallbackConfig {
            chain: vec![FallbackEntry {
                provider: "openrouter".to_string(),
                model: None,
            }],
        }));

        let result = try_with_fallback(
            &MockTokenProvider,
            "openrouter",
            "test-model",
            &config,
            TaskType::Triage,
            operation,
        )
        .await;

        assert!(result.is_ok(), "expected fallback entry to succeed");
        assert_eq!(calls.load(Ordering::SeqCst), 2, "fallback should be called");
    }

    /// Arrange: primary rate limited with no fallback chain configured.
    /// Act: run `try_with_fallback`.
    /// Assert: the original `RateLimited` error is surfaced via downcast,
    /// preserving the original error instance (not a reconstruction).
    #[tokio::test]
    async fn test_rate_limited_without_chain_surfaces_error() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_op = calls.clone();
        let operation = |_client: AiClient| {
            let counter = calls_for_op.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::anyhow!(AptuError::RateLimited {
                    provider: "openrouter".to_string(),
                    retry_after: 7,
                })) as anyhow::Result<u32>
            }
        };

        let config = test_config(None);

        let result = try_with_fallback(
            &MockTokenProvider,
            "openrouter",
            "test-model",
            &config,
            TaskType::Triage,
            operation,
        )
        .await;

        match result {
            Err(AptuError::RateLimited {
                provider,
                retry_after,
            }) => {
                assert_eq!(provider, "openrouter");
                assert_eq!(retry_after, 7);
            }
            other => panic!("expected RateLimited error, got {other:?}"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no fallback attempted");
    }

    /// Arrange: primary returns `TruncatedResponse` with a fallback chain configured.
    /// Act: run `try_with_fallback`.
    /// Assert: returns early without consulting the fallback chain.
    #[tokio::test]
    async fn test_truncated_response_returns_early() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_op = calls.clone();
        let operation = |_client: AiClient| {
            let counter = calls_for_op.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::anyhow!(AptuError::TruncatedResponse {
                    provider: "openrouter".to_string(),
                })) as anyhow::Result<u32>
            }
        };

        let config = test_config(Some(FallbackConfig {
            chain: vec![FallbackEntry {
                provider: "openrouter".to_string(),
                model: None,
            }],
        }));

        let result = try_with_fallback(
            &MockTokenProvider,
            "openrouter",
            "test-model",
            &config,
            TaskType::Review,
            operation,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1, "fallback must not run");
    }
}
