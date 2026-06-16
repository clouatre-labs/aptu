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
    use secrecy::ExposeSecret;
    use serial_test::serial;
    use std::collections::HashMap;

    /// Mock implementation for testing.
    struct MockTokenProvider {
        github_token: Option<SecretString>,
        ai_keys: HashMap<String, SecretString>,
    }

    impl Drop for MockTokenProvider {
        fn drop(&mut self) {
            use zeroize::Zeroize;
            if let Some(ref mut gh_token) = self.github_token {
                gh_token.zeroize();
            }
            for ai_key in self.ai_keys.values_mut() {
                ai_key.zeroize();
            }
        }
    }

    impl TokenProvider for MockTokenProvider {
        fn github_token(&self) -> Option<SecretString> {
            self.github_token.clone()
        }

        fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
            self.ai_keys.get(provider).cloned()
        }
    }

    /// Reads tokens from environment variables at runtime.
    ///
    /// Always available (no cfg gate). On WASM, `std::env::var` returns
    /// `Err(NotPresent)` for all variables, so both methods naturally
    /// return `None` without needing platform gating.
    pub struct EnvTokenProvider;

    impl TokenProvider for EnvTokenProvider {
        fn github_token(&self) -> Option<SecretString> {
            std::env::var("GITHUB_TOKEN")
                .or_else(|_| std::env::var("GH_TOKEN"))
                .ok()
                .map(SecretString::from)
        }

        fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
            let var = format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"));
            std::env::var(&var).ok().map(SecretString::from)
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

    #[test]
    #[serial]
    fn test_env_token_provider_github_token() {
        // SAFETY: single-threaded test process; no concurrent env reads.
        unsafe {
            std::env::set_var("GITHUB_TOKEN", "test_gh_token_abc");
        }
        let provider = EnvTokenProvider;
        let result = provider.github_token();
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
        }
        assert!(result.is_some());
        assert_eq!(result.unwrap().expose_secret(), "test_gh_token_abc");
    }

    #[test]
    #[serial]
    fn test_env_token_provider_ai_api_key() {
        // SAFETY: single-threaded test process; no concurrent env reads.
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "sk-test-key");
        }
        let provider = EnvTokenProvider;
        let result = provider.ai_api_key("openai");
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
        }
        assert!(result.is_some());
        assert_eq!(result.unwrap().expose_secret(), "sk-test-key");
    }

    #[test]
    #[serial]
    fn test_env_token_provider_no_env() {
        // Ensure GITHUB_TOKEN and GH_TOKEN are unset
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
            std::env::remove_var("GH_TOKEN");
        }
        let provider = EnvTokenProvider;
        assert!(provider.github_token().is_none());
    }
}
