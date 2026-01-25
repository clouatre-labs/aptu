// SPDX-License-Identifier: Apache-2.0

//! CLI-specific error formatting with user-friendly hints.
//!
//! This module provides a formatting layer that downcasts `anyhow::Error` to
//! `AptuError` and adds platform-specific hints for different error types.
//! This separates structured error data (library) from user-friendly presentation (CLI),
//! enabling iOS/MCP to format errors appropriately for their platforms.

use std::fmt::Write;

use anyhow::Error;
use aptu_core::error::AptuError;

/// Formats an error for CLI display with helpful hints.
///
/// Downcasts `anyhow::Error` to `AptuError` and adds provider-specific hints.
/// If the error is not an `AptuError`, returns the original error message.
///
/// # Arguments
///
/// * `error` - The error to format
///
/// # Returns
///
/// A formatted error message with hints
#[allow(clippy::too_many_lines)]
pub fn format_error(error: &Error) -> String {
    // Try to downcast to AptuError
    if let Some(aptu_err) = error.downcast_ref::<AptuError>() {
        match aptu_err {
            AptuError::RateLimited {
                provider,
                retry_after,
            } => format_rate_limited_error(provider, *retry_after),
            AptuError::NotAuthenticated => {
                "Authentication required - run `aptu auth login` first".to_string()
            }
            AptuError::AiProviderNotAuthenticated { provider, env_var } => {
                let mut msg = format!("AI provider '{provider}' is not authenticated\n");
                let _ = write!(
                    msg,
                    "\nTo fix this, set the {env_var} environment variable:\n"
                );
                let _ = writeln!(msg, "  export {env_var}=your_api_key_here");
                let _ = write!(msg, "\nThen run your command again.");
                msg
            }
            AptuError::AI {
                message,
                status,
                provider,
            } => {
                let mut msg = format!("AI provider error: {message}");
                if let Some(code) = status {
                    let _ = write!(msg, " (HTTP {code})");
                }

                // Use registry to get provider-specific API key hint
                let api_key_env = aptu_core::ai::registry::get_provider(provider)
                    .map_or("OPENROUTER_API_KEY", |p| p.api_key_env);

                let _ = write!(
                    msg,
                    "\n\nTip: Check your {api_key_env} environment variable."
                );
                msg
            }
            AptuError::Config { message: _ } => {
                format!(
                    "{aptu_err}\n\nTip: Check your config file at {}",
                    aptu_core::config::config_file_path().display()
                )
            }
            AptuError::InvalidAIResponse(_) => {
                format!(
                    "{aptu_err}\n\nTip: This may be a temporary issue with the AI provider. Try again in a moment."
                )
            }
            AptuError::Network(_) => {
                format!("{aptu_err}\n\nTip: Check your internet connection and try again.")
            }
            AptuError::GitHub { message: _ } => {
                format!("{aptu_err}\n\nTip: Check your GitHub token with `aptu auth status`.")
            }
            AptuError::Keyring(_) => {
                format!(
                    "{aptu_err}\n\nTip: Your system keyring may be locked. Try unlocking it and try again."
                )
            }
            AptuError::CircuitOpen => {
                format!(
                    "{aptu_err}\n\nTip: The AI provider is temporarily unavailable. Please try again in a moment."
                )
            }
            AptuError::TruncatedResponse { provider } => {
                format!(
                    "{aptu_err}\n\nTip: The {provider} AI provider returned an incomplete response. This may be due to token limits. Try again in a moment."
                )
            }
            AptuError::TypeMismatch {
                number: _,
                expected: _,
                actual: _,
            } => {
                // Type mismatch errors are clear and actionable - no tip needed
                aptu_err.to_string()
            }
            AptuError::ModelRegistry { message: _ } => {
                format!(
                    "{aptu_err}\n\nTip: Failed to fetch or validate models from the provider API. Check your internet connection and try again."
                )
            }
            AptuError::ModelValidation {
                model_id,
                suggestions,
            } => {
                let mut msg = format!("Invalid model ID: {model_id}");
                if !suggestions.is_empty() {
                    msg.push_str("\n\nDid you mean one of these?");
                    for suggestion in suggestions.split('\n') {
                        if !suggestion.is_empty() {
                            let _ = write!(msg, "\n  - {suggestion}");
                        }
                    }
                }
                msg
            }
            AptuError::SecurityScan { message: _ } => {
                format!("{aptu_err}\n\nTip: Security scan encountered an error. Check the pattern definitions and try again.")
            }
        }
    } else {
        // Not an AptuError, return the original error chain
        error.to_string()
    }
}

/// Formats a rate limit error with provider-specific hints.
fn format_rate_limited_error(provider: &str, retry_after: u64) -> String {
    let mut msg = format!("Rate limit exceeded on {provider}, retry after {retry_after}s");

    if provider == "openrouter" {
        msg.push_str("\n\nTip: You've hit the OpenRouter API rate limit.");
        msg.push_str("\n- Wait at least ");
        let _ = write!(msg, "{retry_after}");
        msg.push_str(" seconds before retrying.");
        msg.push_str("\n- To increase your rate limit, upgrade your OpenRouter account:");
        msg.push_str("\n  https://openrouter.ai/account/limits");
    } else {
        msg.push_str("\n\nTip: You've hit the rate limit for this provider.");
        msg.push_str("\n- Wait at least ");
        let _ = write!(msg, "{retry_after}");
        msg.push_str(" seconds before retrying.");
    }

    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_rate_limited_error_openrouter() {
        let error = AptuError::RateLimited {
            provider: "openrouter".to_string(),
            retry_after: 60,
        };
        let anyhow_err = anyhow::Error::new(error);
        let formatted = format_error(&anyhow_err);

        assert!(formatted.contains("Rate limit exceeded on openrouter"));
        assert!(formatted.contains("60s"));
        assert!(formatted.contains("https://openrouter.ai/account/limits"));
    }

    #[test]
    fn test_format_rate_limited_error_unknown_provider() {
        let error = AptuError::RateLimited {
            provider: "unknown".to_string(),
            retry_after: 30,
        };
        let anyhow_err = anyhow::Error::new(error);
        let formatted = format_error(&anyhow_err);

        assert!(formatted.contains("Rate limit exceeded on unknown"));
        assert!(formatted.contains("30s"));
        assert!(!formatted.contains("openrouter.ai"));
    }

    #[test]
    fn test_format_not_authenticated_error() {
        let error = AptuError::NotAuthenticated;
        let anyhow_err = anyhow::Error::new(error);
        let formatted = format_error(&anyhow_err);

        assert!(formatted.contains("Authentication required"));
        assert!(formatted.contains("aptu auth login"));
    }

    #[test]
    fn test_format_ai_provider_not_authenticated() {
        let error = AptuError::AiProviderNotAuthenticated {
            provider: "openrouter".to_string(),
            env_var: "OPENROUTER_API_KEY".to_string(),
        };
        let anyhow_err = anyhow::Error::new(error);
        let formatted = format_error(&anyhow_err);

        assert!(formatted.contains("AI provider 'openrouter' is not authenticated"));
        assert!(formatted.contains("OPENROUTER_API_KEY"));
        assert!(formatted.contains("export"));
        assert!(formatted.contains("To fix this"));
    }

    #[test]
    fn test_format_ai_error_with_status() {
        let error = AptuError::AI {
            message: "Invalid request".to_string(),
            status: Some(400),
            provider: "openrouter".to_string(),
        };
        let anyhow_err = anyhow::Error::new(error);
        let formatted = format_error(&anyhow_err);

        assert!(formatted.contains("AI provider error"));
        assert!(formatted.contains("Invalid request"));
        assert!(formatted.contains("HTTP 400"));
        assert!(formatted.contains("OPENROUTER_API_KEY"));
    }

    #[test]
    fn test_format_ai_error_without_status() {
        let error = AptuError::AI {
            message: "Connection timeout".to_string(),
            status: None,
            provider: "ollama".to_string(),
        };
        let anyhow_err = anyhow::Error::new(error);
        let formatted = format_error(&anyhow_err);

        assert!(formatted.contains("AI provider error"));
        assert!(formatted.contains("Connection timeout"));
        assert!(!formatted.contains("HTTP"));
    }

    // Note: Network error test omitted - would require reqwest as dev dependency
    // The Network variant formatting is simple and covered by code review

    #[test]
    fn test_format_non_aptu_error() {
        let error = anyhow::anyhow!("Some generic error");
        let formatted = format_error(&error);

        assert_eq!(formatted, "Some generic error");
    }
}
