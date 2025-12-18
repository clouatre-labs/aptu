//! Error types for the Aptu CLI.
//!
//! Uses `thiserror` for deriving `std::error::Error` implementations.
//! Application code should use `anyhow::Result` for top-level error handling.

use thiserror::Error;

/// Errors that can occur during Aptu operations.
#[derive(Error, Debug)]
pub enum AptuError {
    /// GitHub API error from octocrab.
    #[error("GitHub API error: {0}")]
    GitHub(#[from] octocrab::Error),

    /// AI provider error (`OpenRouter`, Ollama, etc.).
    #[error("AI provider error: {message}")]
    AI {
        message: String,
        status: Option<u16>,
    },

    /// User is not authenticated - needs to run `aptu auth login`.
    #[error("Authentication required - run `aptu auth login` first")]
    NotAuthenticated,

    /// GitHub rate limit exceeded.
    #[error("Rate limit exceeded, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    /// Configuration file error.
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    /// Invalid JSON response from AI provider.
    #[error("Invalid JSON response from AI")]
    InvalidAIResponse(#[source] serde_json::Error),

    /// Network/HTTP error from reqwest.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Keyring/credential storage error.
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),
}
