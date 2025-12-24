// SPDX-License-Identifier: Apache-2.0

//! Token provider abstraction for multi-platform credential resolution.
//!
//! This module defines the `TokenProvider` trait, which abstracts credential
//! resolution across different platforms (CLI, iOS, etc.). Each platform
//! implements this trait to provide GitHub and AI provider tokens from their
//! respective credential sources.

use secrecy::SecretString;

/// Provides GitHub and AI provider credentials for API calls.
///
/// This trait abstracts credential resolution across platforms:
/// - **CLI:** Resolves from environment variables, GitHub CLI, or system keyring
/// - **iOS:** Resolves from iOS keychain via FFI
///
/// Implementations should handle credential lookup and return `None` if
/// credentials are not available.
pub trait TokenProvider: Send + Sync {
    /// Retrieves the GitHub API token.
    ///
    /// Returns `None` if no token is available from any source.
    fn github_token(&self) -> Option<SecretString>;

    /// Retrieves an AI provider API key.
    ///
    /// # Arguments
    /// * `provider` - The AI provider name (must match a registered provider in the registry)
    ///
    /// Returns `None` if no API key is available from any source.
    fn ai_api_key(&self, provider: &str) -> Option<SecretString>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::registry::all_providers;
    use std::collections::HashMap;

    /// Mock implementation for testing.
    struct MockTokenProvider {
        github_token: Option<SecretString>,
        ai_keys: HashMap<String, SecretString>,
    }

    impl TokenProvider for MockTokenProvider {
        fn github_token(&self) -> Option<SecretString> {
            self.github_token.clone()
        }

        fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
            self.ai_keys.get(provider).cloned()
        }
    }

    #[test]
    fn test_mock_provider_with_tokens() {
        let mut ai_keys = HashMap::new();
        for provider_config in all_providers() {
            ai_keys.insert(
                provider_config.name.to_string(),
                SecretString::from(format!("{}_key", provider_config.name)),
            );
        }

        let provider = MockTokenProvider {
            github_token: Some(SecretString::from("gh_token")),
            ai_keys,
        };

        assert!(provider.github_token().is_some());
        for provider_config in all_providers() {
            assert!(
                provider.ai_api_key(provider_config.name).is_some(),
                "Expected key for provider: {}",
                provider_config.name
            );
        }
    }

    #[test]
    fn test_mock_provider_without_tokens() {
        let provider = MockTokenProvider {
            github_token: None,
            ai_keys: HashMap::new(),
        };

        assert!(provider.github_token().is_none());
        for provider_config in all_providers() {
            assert!(
                provider.ai_api_key(provider_config.name).is_none(),
                "Expected no key for provider: {}",
                provider_config.name
            );
        }
    }
}
