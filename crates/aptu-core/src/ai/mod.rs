// SPDX-License-Identifier: Apache-2.0

//! AI integration module.
//!
//! Provides AI-assisted issue triage using multiple AI providers (Gemini, `OpenRouter`, Groq, Cerebras).

pub mod circuit_breaker;
pub mod client;
pub mod models;
pub mod provider;
pub mod registry;
pub mod types;

pub use circuit_breaker::CircuitBreaker;
pub use client::AiClient;
pub use models::{AiModel, ModelProvider};
pub use provider::AiProvider;
pub use registry::{ModelInfo, ProviderConfig, all_providers, get_provider};
pub use types::{CreateIssueResponse, CreditsStatus, TriageResponse};

use crate::history::AiStats;

/// Cerebras API base URL (OpenAI-compatible endpoint).
pub const CEREBRAS_API_URL: &str = "https://api.cerebras.ai/v1/chat/completions";

/// Environment variable for Cerebras API key.
pub const CEREBRAS_API_KEY_ENV: &str = "CEREBRAS_API_KEY";

/// Gemini API base URL (OpenAI-compatible endpoint).
pub const GEMINI_API_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions";

/// Environment variable for Gemini API key.
pub const GEMINI_API_KEY_ENV: &str = "GEMINI_API_KEY";

/// Groq API base URL (OpenAI-compatible endpoint).
pub const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

/// Environment variable for Groq API key.
pub const GROQ_API_KEY_ENV: &str = "GROQ_API_KEY";

/// `OpenRouter` API base URL.
pub const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Environment variable for `OpenRouter` API key.
pub const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

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
