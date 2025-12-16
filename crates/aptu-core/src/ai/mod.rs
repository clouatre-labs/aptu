//! AI integration module.
//!
//! Provides AI-assisted issue triage using `OpenRouter` API.

pub mod openrouter;
pub mod types;

/// `OpenRouter` API base URL.
pub const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Environment variable for `OpenRouter` API key.
pub const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

/// Checks if a model is in the free tier (no cost).
/// Free models on `OpenRouter` always have the `:free` suffix.
#[must_use]
pub fn is_free_model(model: &str) -> bool {
    model.ends_with(":free")
}
