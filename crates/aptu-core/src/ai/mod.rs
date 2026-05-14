// SPDX-License-Identifier: Apache-2.0

//! AI integration module.
//!
//! Provides AI-assisted issue triage using multiple AI providers (Gemini, `OpenRouter`, Groq, Cerebras, Zenmux, Z.AI).

pub mod circuit_breaker;
pub mod client;
pub mod context;
pub mod models;
pub mod prompts;
pub mod provider;
pub mod registry;
pub mod types;

pub use circuit_breaker::CircuitBreaker;
pub use client::{AiClient, AuthMethod};
pub use models::{AiModel, ModelProvider};
pub use provider::AiProvider;
pub use registry::{ProviderConfig, all_providers, get_provider};
pub use types::{CreateIssueResponse, CreditsStatus, TriageResponse};

use crate::history::AiStats;

/// Response from AI analysis containing both triage data and usage stats.
#[derive(Debug, Clone)]
pub struct AiResponse {
    /// The triage analysis result.
    pub triage: TriageResponse,
    /// AI usage statistics.
    pub stats: AiStats,
}

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
pub(crate) fn resolve_anthropic_credential(
    ai_config: &crate::config::AiConfig,
) -> Option<AiClient> {
    // Try keyring first
    if let Ok(Some(client)) = AiClient::from_keyring_oauth(ai_config) {
        return Some(client);
    }

    // Try credentials file
    if let Ok(Some(client)) = AiClient::from_claude_credentials(ai_config) {
        return Some(client);
    }

    // Fall back to environment variable
    AiClient::new("anthropic", ai_config).ok()
}

/// Sets up the primary AI client with credential resolution.
///
/// For the Anthropic provider, attempts to use Claude OAuth credentials in this order:
/// 1. Existing token in OS keyring
/// 2. ~/.claude/credentials.json file
/// 3. Environment variable (fallback)
///
/// For other providers, uses the standard environment variable path.
///
/// # Errors
///
/// Returns an error if client creation fails.
pub fn setup_primary_client(config: &crate::config::AppConfig) -> anyhow::Result<AiClient> {
    // For Anthropic, delegate to centralized credential resolution
    if config.ai.provider == "anthropic"
        && let Some(client) = resolve_anthropic_credential(&config.ai)
    {
        return Ok(client);
    }

    // Fall back to environment variable for non-Anthropic providers
    AiClient::new(&config.ai.provider, &config.ai)
}

/// Creates a formatted GitHub issue using AI assistance.
///
/// Takes raw issue title and body, formats them professionally using the configured AI provider.
/// Returns formatted title, body, and suggested labels.
///
/// # Arguments
///
/// * `title` - Raw issue title from user
/// * `body` - Raw issue body/description from user
/// * `repo` - Repository name for context (owner/repo format)
///
/// # Errors
///
/// Returns an error if AI formatting fails or API is unavailable.
pub async fn create_issue(
    title: &str,
    body: &str,
    repo: &str,
) -> anyhow::Result<(CreateIssueResponse, AiStats)> {
    let config = crate::config::load_config()?;

    // Create generic client for the configured provider
    let client = setup_primary_client(&config)?;
    client.create_issue(title, body, repo).await
}
