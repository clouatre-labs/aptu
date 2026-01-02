// SPDX-License-Identifier: Apache-2.0

//! Error types for the Aptu CLI.
//!
//! Uses `thiserror` for deriving `std::error::Error` implementations.
//! Application code should use `anyhow::Result` for top-level error handling.

use thiserror::Error;

/// Errors that can occur during Aptu operations.
#[derive(Error, Debug)]
pub enum AptuError {
    /// GitHub API error from octocrab.
    #[error("GitHub API error: {message}")]
    GitHub {
        /// Error message.
        message: String,
    },

    /// AI provider error (`OpenRouter`, Ollama, etc.).
    #[error("AI provider error: {message}")]
    AI {
        /// Error message from the AI provider.
        message: String,
        /// Optional HTTP status code from the provider.
        status: Option<u16>,
        /// Name of the AI provider (e.g., `OpenRouter`, `Ollama`).
        provider: String,
    },

    /// User is not authenticated - needs to run `aptu auth login`.
    #[error(
        "Authentication required - run `aptu auth login` first, or set GITHUB_TOKEN environment variable"
    )]
    NotAuthenticated,

    /// Rate limit exceeded from an AI provider.
    #[error("Rate limit exceeded on {provider}, retry after {retry_after}s")]
    RateLimited {
        /// Name of the provider that rate limited (e.g., `OpenRouter`).
        provider: String,
        /// Number of seconds to wait before retrying.
        retry_after: u64,
    },

    /// AI response was truncated (incomplete JSON due to EOF).
    #[error("Truncated response from {provider} - response ended prematurely")]
    TruncatedResponse {
        /// Name of the AI provider that returned truncated response.
        provider: String,
    },

    /// Configuration file error.
    #[error("Configuration error: {message}")]
    Config {
        /// Error message.
        message: String,
    },

    /// Invalid JSON response from AI provider.
    #[error("Invalid JSON response from AI")]
    InvalidAIResponse(#[source] serde_json::Error),

    /// Network/HTTP error from reqwest.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Keyring/credential storage error.
    #[cfg(feature = "keyring")]
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    /// Circuit breaker is open - AI provider is unavailable.
    #[error("Circuit breaker is open - AI provider is temporarily unavailable")]
    CircuitOpen,

    /// Type mismatch: reference is a different type than expected.
    #[error("#{number} is {actual}, not {expected}")]
    TypeMismatch {
        /// The issue/PR number.
        number: u64,
        /// Expected type.
        expected: ResourceType,
        /// Actual type.
        actual: ResourceType,
    },

    /// Model registry error (runtime model validation).
    #[error("Model registry error: {message}")]
    ModelRegistry {
        /// Error message.
        message: String,
    },

    /// Model validation error - invalid model ID with suggestions.
    #[error("Invalid model ID: {model_id}. Did you mean one of these?\n{suggestions}")]
    ModelValidation {
        /// The invalid model ID provided by the user.
        model_id: String,
        /// Suggested valid model IDs based on fuzzy matching.
        suggestions: String,
    },
}

/// GitHub resource type for type mismatch errors.
#[derive(Debug, Clone, Copy)]
pub enum ResourceType {
    /// GitHub issue.
    Issue,
    /// GitHub pull request.
    PullRequest,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::Issue => write!(f, "issue"),
            ResourceType::PullRequest => write!(f, "pull request"),
        }
    }
}

impl From<octocrab::Error> for AptuError {
    fn from(err: octocrab::Error) -> Self {
        AptuError::GitHub {
            message: err.to_string(),
        }
    }
}

impl From<config::ConfigError> for AptuError {
    fn from(err: config::ConfigError) -> Self {
        AptuError::Config {
            message: err.to_string(),
        }
    }
}
