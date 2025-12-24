// SPDX-License-Identifier: Apache-2.0

//! CLI-specific `TokenProvider` implementation.
//!
//! Provides GitHub, `OpenRouter`, Gemini, Groq, and Cerebras credentials for CLI commands by resolving
//! tokens from environment variables, GitHub CLI, system keyring, and
//! environment variables for `OpenRouter`, Gemini, Groq, and Cerebras API keys.

use aptu_core::auth::TokenProvider;
use secrecy::SecretString;
use tracing::debug;

/// CLI implementation of `TokenProvider`.
///
/// Resolves credentials from:
/// - GitHub: Environment variables, GitHub CLI, or system keyring
/// - `OpenRouter`: `OPENROUTER_API_KEY` environment variable
/// - Gemini: `GEMINI_API_KEY` environment variable
/// - Groq: `GROQ_API_KEY` environment variable
/// - Cerebras: `CEREBRAS_API_KEY` environment variable
pub struct CliTokenProvider;

impl TokenProvider for CliTokenProvider {
    fn github_token(&self) -> Option<SecretString> {
        if let Some((token, _source)) = aptu_core::github::auth::resolve_token() {
            debug!("Resolved GitHub token from CLI sources");
            Some(token)
        } else {
            debug!("No GitHub token found in CLI sources");
            None
        }
    }

    fn cerebras_key(&self) -> Option<SecretString> {
        match std::env::var("CEREBRAS_API_KEY") {
            Ok(key) if !key.is_empty() => {
                debug!("Resolved Cerebras API key from environment variable");
                Some(SecretString::from(key))
            }
            _ => {
                debug!("No Cerebras API key found in environment");
                None
            }
        }
    }

    fn gemini_key(&self) -> Option<SecretString> {
        match std::env::var("GEMINI_API_KEY") {
            Ok(key) if !key.is_empty() => {
                debug!("Resolved Gemini API key from environment variable");
                Some(SecretString::from(key))
            }
            _ => {
                debug!("No Gemini API key found in environment");
                None
            }
        }
    }

    fn groq_key(&self) -> Option<SecretString> {
        match std::env::var("GROQ_API_KEY") {
            Ok(key) if !key.is_empty() => {
                debug!("Resolved Groq API key from environment variable");
                Some(SecretString::from(key))
            }
            _ => {
                debug!("No Groq API key found in environment");
                None
            }
        }
    }

    fn openrouter_key(&self) -> Option<SecretString> {
        match std::env::var("OPENROUTER_API_KEY") {
            Ok(key) if !key.is_empty() => {
                debug!("Resolved OpenRouter API key from environment variable");
                Some(SecretString::from(key))
            }
            _ => {
                debug!("No OpenRouter API key found in environment");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_provider_creation() {
        let provider = CliTokenProvider;
        // Just verify we can create an instance
        let _ = provider;
    }
}
