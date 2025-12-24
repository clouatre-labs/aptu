// SPDX-License-Identifier: Apache-2.0

//! `OpenRouter` API client for AI-assisted issue triage.
//!
//! Provides functionality to analyze GitHub issues using the `OpenRouter` API
//! with structured JSON output.

use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::SecretString;

use super::provider::AiProvider;
use super::{OPENROUTER_API_KEY_ENV, OPENROUTER_API_URL};
use crate::config::AiConfig;

/// `OpenRouter` account credits status.
#[derive(Debug, Clone)]
pub struct CreditsStatus {
    /// Available credits in USD.
    pub credits: f64,
}

impl CreditsStatus {
    /// Returns a human-readable status message.
    #[must_use]
    pub fn message(&self) -> String {
        format!("OpenRouter credits: ${:.4}", self.credits)
    }
}

/// `OpenRouter` API client for issue triage.
///
/// Holds HTTP client, API key, and model configuration for reuse across multiple requests.
/// Enables connection pooling and cleaner API.
pub struct OpenRouterClient {
    /// HTTP client with configured timeout.
    http: Client,
    /// API key for `OpenRouter` authentication.
    api_key: SecretString,
    /// Model name (e.g., "mistralai/devstral-2512:free").
    model: String,
}

impl OpenRouterClient {
    /// Creates a new `OpenRouter` client from configuration.
    ///
    /// Validates the model against cost control settings and fetches the API key
    /// from the environment.
    ///
    /// # Arguments
    ///
    /// * `config` - AI configuration with model, timeout, and cost control settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Model is not in free tier and `allow_paid_models` is false
    /// - `OPENROUTER_API_KEY` environment variable is not set
    /// - HTTP client creation fails
    pub fn new(config: &AiConfig) -> Result<Self> {
        // Validate model against cost control
        if !config.allow_paid_models && !super::is_free_model(&config.model) {
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
        let api_key = env::var(OPENROUTER_API_KEY_ENV).with_context(|| {
            format!(
                "Missing {OPENROUTER_API_KEY_ENV} environment variable.\n\
                 Set it with: export {OPENROUTER_API_KEY_ENV}=your_api_key\n\
                 Get a free key at: https://openrouter.ai/keys"
            )
        })?;

        // Create HTTP client with timeout
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            http,
            api_key: SecretString::new(api_key.into()),
            model: config.model.clone(),
        })
    }

    /// Creates a new `OpenRouter` client with a provided API key.
    ///
    /// This constructor allows callers to provide an API key directly,
    /// enabling multi-platform credential resolution (e.g., from iOS keychain via FFI).
    ///
    /// # Arguments
    ///
    /// * `api_key` - `OpenRouter` API key as a `SecretString`
    /// * `config` - AI configuration with model, timeout, and cost control settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Model is not in free tier and `allow_paid_models` is false
    /// - HTTP client creation fails
    pub fn with_api_key(api_key: SecretString, config: &AiConfig) -> Result<Self> {
        // Validate model against cost control
        if !config.allow_paid_models && !super::is_free_model(&config.model) {
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
            http,
            api_key,
            model: config.model.clone(),
        })
    }
}

#[async_trait]
impl AiProvider for OpenRouterClient {
    fn name(&self) -> &'static str {
        "openrouter"
    }

    fn api_url(&self) -> &str {
        OPENROUTER_API_URL
    }

    fn api_key_env(&self) -> &str {
        OPENROUTER_API_KEY_ENV
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

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(val) = "application/json".parse() {
            headers.insert("Content-Type", val);
        }
        if let Ok(val) = "https://github.com/clouatre-labs/project-aptu".parse() {
            headers.insert("HTTP-Referer", val);
        }
        if let Ok(val) = "Aptu CLI".parse() {
            headers.insert("X-Title", val);
        }
        headers
    }
}
