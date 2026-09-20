// SPDX-License-Identifier: Apache-2.0

//! Error types for the aptu-core library.
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

    /// AI provider is not authenticated - missing API key.
    #[error("AI provider '{provider}' is not authenticated - set {env_var} environment variable")]
    AiProviderNotAuthenticated {
        /// Name of the AI provider (e.g., `OpenRouter`, `Ollama`).
        provider: String,
        /// Environment variable name to set (e.g., `OPENROUTER_API_KEY`).
        env_var: String,
    },

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
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Keyring/credential storage error.
    #[cfg(feature = "keyring")]
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring_core::error::Error),

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

    /// Security scan error.
    #[error("Security scan error: {message}")]
    SecurityScan {
        /// Error message.
        message: String,
    },

    /// A user-supplied input field exceeds its configured byte limit.
    #[error(
        "input field `{field}` exceeds limit: {actual_bytes} bytes (limit: {limit_bytes} bytes){hint}"
    )]
    InputExceedsLimit {
        /// Name of the field that exceeded the limit.
        field: String,
        /// Actual byte count of the input.
        actual_bytes: usize,
        /// Configured byte limit.
        limit_bytes: usize,
        /// Optional hint for how to resolve the limit (empty if none).
        hint: String,
    },

    /// The authenticated viewer lacks write access to the target resource.
    #[error("Permission denied for {resource}: {message}")]
    PermissionDenied {
        /// Name of the resource access was denied for (e.g. `owner/repo`).
        resource: String,
        /// Error message describing the denial.
        message: String,
    },

    /// Review context ended up with zero surviving file patches on a non-empty PR,
    /// which would produce a diff-less review that GitHub rejects.
    #[error(
        "review context has no surviving patches across {files_total} file(s); refusing to submit a diff-less review"
    )]
    EmptyReviewContext {
        /// Total number of files in the PR.
        files_total: usize,
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

/// Returns a typed [`AptuError::PermissionDenied`] for GitHub 403/404 statuses.
///
/// Any other status maps to `None` and callers should fall back to the generic
/// [`AptuError::GitHub`] mapping.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn permission_denied_for_status(
    status: u16,
    resource: &str,
    message: &str,
) -> Option<AptuError> {
    if status == 403 || status == 404 {
        Some(AptuError::PermissionDenied {
            resource: resource.to_string(),
            message: message.to_string(),
        })
    } else {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<octocrab::Error> for AptuError {
    fn from(err: octocrab::Error) -> Self {
        if let octocrab::Error::GitHub { source, .. } = &err {
            let status = source.status_code.as_u16();
            let resource = source.message.clone();
            if let Some(denied) = permission_denied_for_status(status, &resource, &err.to_string())
            {
                return denied;
            }
        }
        AptuError::GitHub {
            message: err.to_string(),
        }
    }
}

/// Maps an `anyhow::Error` into an [`AptuError`], preserving the typed
/// [`AptuError::PermissionDenied`] mapping when the underlying error is an
/// octocrab error with a 403/404 status.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn aptu_error_from_anyhow(err: anyhow::Error) -> AptuError {
    match err.downcast::<octocrab::Error>() {
        Ok(octo_err) => octo_err.into(),
        Err(other) => AptuError::GitHub {
            message: other.to_string(),
        },
    }
}

impl From<config::ConfigError> for AptuError {
    fn from(err: config::ConfigError) -> Self {
        AptuError::Config {
            message: err.to_string(),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{AptuError, permission_denied_for_status};

    #[test]
    fn test_403_maps_to_permission_denied() {
        let err = permission_denied_for_status(403, "owner/repo", "forbidden")
            .expect("403 should map to PermissionDenied");
        match err {
            AptuError::PermissionDenied { resource, message } => {
                assert_eq!(resource, "owner/repo");
                assert_eq!(message, "forbidden");
            }
            other => panic!("Expected PermissionDenied, got: {other:?}"),
        }
    }

    #[test]
    fn test_404_maps_and_other_statuses_do_not() {
        assert!(permission_denied_for_status(404, "owner/repo", "not found").is_some());
        assert!(permission_denied_for_status(500, "owner/repo", "oops").is_none());
        assert!(permission_denied_for_status(422, "owner/repo", "invalid").is_none());
    }

    #[test]
    fn anyhow_wrapped_octocrab_error_downcasts_through_helper() {
        // Uses the constructible Uri variant to prove the downcast path in
        // aptu_error_from_anyhow maps octocrab errors through From rather
        // than stringifying them; the 403->PermissionDenied status mapping
        // itself is covered by permission_denied_for_status tests above and
        // the mock-server integration test in tests/graphql_contract.rs.
        let err = octocrab::Error::Other {
            source: Box::new(std::io::Error::other("boom")),
            backtrace: std::backtrace::Backtrace::capture(),
        };
        let mapped = super::aptu_error_from_anyhow(anyhow::anyhow!(err));
        assert!(matches!(mapped, AptuError::GitHub { .. }));
    }
}
