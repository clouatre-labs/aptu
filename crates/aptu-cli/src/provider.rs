//! CLI-specific `TokenProvider` implementation.
//!
//! Provides GitHub and `OpenRouter` credentials for CLI commands by resolving
//! tokens from environment variables, GitHub CLI, system keyring, and
//! environment variables for `OpenRouter` API keys.

use aptu_core::auth::TokenProvider;
use secrecy::SecretString;
use tracing::debug;

/// CLI implementation of `TokenProvider`.
///
/// Resolves credentials from:
/// - GitHub: Environment variables, GitHub CLI, or system keyring
/// - `OpenRouter`: `OPENROUTER_API_KEY` environment variable
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
