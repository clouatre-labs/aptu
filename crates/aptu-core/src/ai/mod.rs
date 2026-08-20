// SPDX-License-Identifier: Apache-2.0

//! AI integration module.
//!
//! Provides AI-assisted issue triage using multiple AI providers (Gemini, `OpenRouter`, Groq, Cerebras, Zenmux, Z.AI).

pub mod circuit_breaker;
pub mod client;
pub mod context;
pub mod dep_enrichment;
pub mod models;
pub mod prompts;
pub mod provider;
pub mod registry;
pub mod review_context;
pub mod types;

pub use circuit_breaker::CircuitBreaker;
pub use client::{AiClient, AuthMethod, is_free_model, resolve_anthropic_credential};
pub use dep_enrichment::enrich_dep_releases;
pub use models::{AiModel, ModelProvider};
pub use provider::AiProvider;
pub use registry::{PROVIDER_ANTHROPIC, ProviderConfig, all_providers, get_provider};
pub use types::{CreateIssueResponse, CreditsStatus, DepReleaseNote, TriageResponse};

use crate::history::AiStats;

/// Response from AI analysis containing both triage data and usage stats.
#[derive(Debug, Clone)]
pub struct AiResponse {
    /// The triage analysis result.
    pub triage: TriageResponse,
    /// AI usage statistics.
    pub stats: AiStats,
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
    if config.ai.provider == PROVIDER_ANTHROPIC
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
#[cfg(not(target_arch = "wasm32"))]
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
