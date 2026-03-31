// SPDX-License-Identifier: Apache-2.0

//! AI integration module.
//!
//! Provides AI-assisted issue triage using multiple AI providers (Gemini, `OpenRouter`, Groq, Cerebras).

pub mod circuit_breaker;
pub mod client;
pub mod context;
pub mod models;
pub mod prompts;
pub mod provider;
pub mod registry;
pub mod types;

pub use circuit_breaker::CircuitBreaker;
pub use client::AiClient;
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
    let client = AiClient::new(&config.ai.provider, &config.ai)?;
    client.create_issue(title, body, repo).await
}
