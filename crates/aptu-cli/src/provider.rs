// SPDX-License-Identifier: Apache-2.0

//! CLI-specific `TokenProvider` implementation.
//!
//! Provides GitHub and AI provider credentials for CLI commands by resolving
//! tokens from environment variables, GitHub CLI, system keyring, and
//! environment variables for AI provider API keys.

use aptu_core::ai::registry;
use aptu_core::auth::TokenProvider;
use secrecy::SecretString;
use tracing::debug;

/// CLI implementation of `TokenProvider`.
///
/// Resolves credentials from:
/// - GitHub: Environment variables, GitHub CLI, or system keyring
/// - AI Providers: Environment variables specified in the provider registry
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

    fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
        let provider_config = registry::get_provider(provider)?;
        match std::env::var(provider_config.api_key_env) {
            Ok(key) if !key.is_empty() => {
                debug!(
                    "Resolved {} API key from environment variable {}",
                    provider, provider_config.api_key_env
                );
                Some(SecretString::from(key))
            }
            _ => {
                debug!(
                    "No {} API key found in environment variable {}",
                    provider, provider_config.api_key_env
                );
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
