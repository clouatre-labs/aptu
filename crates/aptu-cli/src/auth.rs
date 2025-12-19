//! CLI implementation of `TokenProvider` using the token resolution chain.
//!
//! This module provides the CLI's credential resolution strategy:
//! 1. Environment variables (`GH_TOKEN`, `GITHUB_TOKEN`, `OPENROUTER_API_KEY`)
//! 2. GitHub CLI (`gh auth token`)
//! 3. System keyring (native aptu auth)

use aptu_core::TokenProvider;
use secrecy::SecretString;
use std::env;
use tracing::debug;

/// CLI token provider using the standard resolution chain.
///
/// Resolves GitHub and `OpenRouter` credentials from:
/// - Environment variables
/// - GitHub CLI
/// - System keyring
#[allow(dead_code)] // Will be used when CLI commands are updated to use TokenProvider
pub struct CliTokenProvider;

impl TokenProvider for CliTokenProvider {
    fn github_token(&self) -> Option<SecretString> {
        // Try environment variables first
        if let Ok(token) = env::var("GH_TOKEN")
            && !token.is_empty()
        {
            debug!("Using GitHub token from GH_TOKEN environment variable");
            return Some(SecretString::new(token.into()));
        }

        if let Ok(token) = env::var("GITHUB_TOKEN")
            && !token.is_empty()
        {
            debug!("Using GitHub token from GITHUB_TOKEN environment variable");
            return Some(SecretString::new(token.into()));
        }

        // Try GitHub CLI
        if let Some(token) = aptu_core::github::auth::resolve_token() {
            debug!("Using GitHub token from resolution chain");
            return Some(token.0);
        }

        debug!("No GitHub token found");
        None
    }

    fn openrouter_key(&self) -> Option<SecretString> {
        // Try environment variable
        if let Ok(key) = env::var("OPENROUTER_API_KEY")
            && !key.is_empty()
        {
            debug!("Using OpenRouter API key from OPENROUTER_API_KEY environment variable");
            return Some(SecretString::new(key.into()));
        }

        debug!("No OpenRouter API key found");
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_token_provider_implements_trait() {
        let provider = CliTokenProvider;
        // Just verify it implements the trait
        let _: &dyn TokenProvider = &provider;
    }
}
