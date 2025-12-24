// SPDX-License-Identifier: Apache-2.0

//! Google AI Studio (Gemini) API client for AI-assisted issue triage.
//!
//! Provides functionality to analyze GitHub issues using the Google AI Studio API
//! with structured JSON output. Uses OpenAI-compatible endpoint for seamless integration.

use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::SecretString;

use super::provider::AiProvider;
use super::{GEMINI_API_KEY_ENV, GEMINI_API_URL};
use crate::config::AiConfig;

/// Google AI Studio (Gemini) API client for issue triage.
///
/// Holds HTTP client, API key, and model configuration for reuse across multiple requests.
/// Enables connection pooling and cleaner API.
pub struct GeminiClient {
    /// HTTP client with configured timeout.
    http: Client,
    /// API key for Gemini authentication.
    api_key: SecretString,
    /// Model name (e.g., "gemini-3-flash-preview").
    model: String,
    /// Maximum tokens for API responses.
    max_tokens: u32,
    /// Temperature for API requests.
    temperature: f32,
}

impl GeminiClient {
    /// Creates a new Gemini client from configuration.
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
    /// - `GEMINI_API_KEY` environment variable is not set
    /// - HTTP client creation fails
    pub fn new(config: &AiConfig) -> Result<Self> {
        // Get API key from environment
        let api_key = env::var(GEMINI_API_KEY_ENV).with_context(|| {
            format!(
                "Missing {GEMINI_API_KEY_ENV} environment variable.\n\
                 Set it with: export {GEMINI_API_KEY_ENV}=your_api_key\n\
                 Get a free key at: https://aistudio.google.com/app/apikey"
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

    /// Creates a new Gemini client with a provided API key.
    ///
    /// This constructor allows callers to provide an API key directly,
    /// enabling multi-platform credential resolution (e.g., from iOS keychain via FFI).
    ///
    /// # Arguments
    ///
    /// * `api_key` - Gemini API key as a `SecretString`
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
impl AiProvider for GeminiClient {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn api_url(&self) -> &str {
        GEMINI_API_URL
    }

    fn api_key_env(&self) -> &str {
        GEMINI_API_KEY_ENV
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
