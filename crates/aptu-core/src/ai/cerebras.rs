// SPDX-License-Identifier: Apache-2.0

//! Cerebras API client for AI-assisted issue triage.
//!
//! Provides functionality to analyze GitHub issues using the Cerebras API
//! with structured JSON output. Uses OpenAI-compatible endpoint for seamless integration.

use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::SecretString;

use super::provider::AiProvider;
use super::{CEREBRAS_API_KEY_ENV, CEREBRAS_API_URL};
use crate::config::AiConfig;

/// Cerebras API client for issue triage.
///
/// Holds HTTP client, API key, and model configuration for reuse across multiple requests.
/// Enables connection pooling and cleaner API.
pub struct CerebrasClient {
    /// HTTP client with configured timeout.
    http: Client,
    /// API key for Cerebras authentication.
    api_key: SecretString,
    /// Model name (e.g., "llama3.1-8b").
    model: String,
    /// Maximum tokens for API responses.
    max_tokens: u32,
    /// Temperature for API requests.
    temperature: f32,
}

impl CerebrasClient {
    /// Creates a new Cerebras client from configuration.
    ///
    /// Validates the model and fetches the API key from the environment.
    ///
    /// # Arguments
    ///
    /// * `config` - AI configuration with model, timeout, and cost control settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `CEREBRAS_API_KEY` environment variable is not set
    /// - HTTP client creation fails
    pub fn new(config: &AiConfig) -> Result<Self> {
        // Get API key from environment
        let api_key = env::var(CEREBRAS_API_KEY_ENV).with_context(|| {
            format!(
                "Missing {CEREBRAS_API_KEY_ENV} environment variable.\n\
                 Set it with: export {CEREBRAS_API_KEY_ENV}=your_api_key\n\
                 Get a free key at: https://cloud.cerebras.ai/api-keys"
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
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        })
    }

    /// Creates a new Cerebras client with a provided API key.
    ///
    /// This constructor allows callers to provide an API key directly,
    /// enabling multi-platform credential resolution (e.g., from iOS keychain via FFI).
    ///
    /// # Arguments
    ///
    /// * `api_key` - Cerebras API key as a `SecretString`
    /// * `config` - AI configuration with model, timeout, and cost control settings
    ///
    /// # Errors
    ///
    /// Returns an error if HTTP client creation fails
    pub fn with_api_key(api_key: SecretString, config: &AiConfig) -> Result<Self> {
        // Create HTTP client with timeout
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            http,
            api_key,
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        })
    }
}

#[async_trait]
impl AiProvider for CerebrasClient {
    fn name(&self) -> &'static str {
        "cerebras"
    }

    fn api_url(&self) -> &str {
        CEREBRAS_API_URL
    }

    fn api_key_env(&self) -> &str {
        CEREBRAS_API_KEY_ENV
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
}
