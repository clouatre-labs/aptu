// SPDX-License-Identifier: Apache-2.0

//! AI integration module.
//!
//! Provides AI-assisted issue triage using `OpenRouter` API.

pub mod models;
pub mod openrouter;
pub mod types;

pub use models::{AiModel, ModelProvider};
pub use openrouter::OpenRouterClient;
pub use types::TriageResponse;

use crate::history::AiStats;

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

/// Calculate cost in USD based on model pricing and token usage.
///
/// Returns 0.0 for free models (those ending with `:free`).
/// For paid models, uses approximate pricing per 1M tokens.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn calculate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    if is_free_model(model) {
        return 0.0;
    }

    // Approximate pricing per 1M tokens (as of Dec 2024)
    // These are rough estimates and should be updated periodically
    let (input_price, output_price) = match model {
        m if m.starts_with("anthropic/claude-3.5-sonnet") => (3.0, 15.0),
        m if m.starts_with("anthropic/claude-3-opus") => (15.0, 75.0),
        m if m.starts_with("anthropic/claude-3-sonnet") => (3.0, 15.0),
        m if m.starts_with("anthropic/claude-3-haiku") => (0.25, 1.25),
        m if m.starts_with("openai/gpt-4o") => (2.5, 10.0),
        m if m.starts_with("openai/gpt-4-turbo") => (10.0, 30.0),
        m if m.starts_with("openai/gpt-4") => (30.0, 60.0),
        m if m.starts_with("openai/gpt-3.5-turbo") => (0.5, 1.5),
        m if m.starts_with("google/gemini-pro") => (0.5, 1.5),
        m if m.starts_with("mistralai/mistral-large") => (4.0, 12.0),
        m if m.starts_with("mistralai/mistral-medium") => (2.7, 8.1),
        m if m.starts_with("mistralai/mistral-small") => (1.0, 3.0),
        _ => (1.0, 3.0), // Default fallback pricing
    };

    let input_cost = (input_tokens as f64 / 1_000_000.0) * input_price;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * output_price;

    input_cost + output_cost
}
