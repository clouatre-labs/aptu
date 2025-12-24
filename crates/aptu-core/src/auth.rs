// SPDX-License-Identifier: Apache-2.0

//! Token provider abstraction for multi-platform credential resolution.
//!
//! This module defines the `TokenProvider` trait, which abstracts credential
//! resolution across different platforms (CLI, iOS, etc.). Each platform
//! implements this trait to provide GitHub, `OpenRouter`, Gemini, Groq, and Cerebras tokens from their
//! respective credential sources.

use secrecy::SecretString;

/// Provides GitHub, `OpenRouter`, Gemini, Groq, and Cerebras credentials for API calls.
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

    /// Retrieves the Cerebras API key.
    ///
    /// Returns `None` if no API key is available from any source.
    fn cerebras_key(&self) -> Option<SecretString>;

    /// Retrieves the Gemini API key.
    ///
    /// Returns `None` if no API key is available from any source.
    fn gemini_key(&self) -> Option<SecretString>;

    /// Retrieves the Groq API key.
    ///
    /// Returns `None` if no API key is available from any source.
    fn groq_key(&self) -> Option<SecretString>;

    /// Retrieves the `OpenRouter` API key.
    ///
    /// Returns `None` if no API key is available from any source.
    fn openrouter_key(&self) -> Option<SecretString>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock implementation for testing.
    struct MockTokenProvider {
        github_token: Option<SecretString>,
        cerebras_key: Option<SecretString>,
        gemini_key: Option<SecretString>,
        groq_key: Option<SecretString>,
        openrouter_key: Option<SecretString>,
    }

    impl TokenProvider for MockTokenProvider {
        fn github_token(&self) -> Option<SecretString> {
            self.github_token.clone()
        }

        fn cerebras_key(&self) -> Option<SecretString> {
            self.cerebras_key.clone()
        }

        fn gemini_key(&self) -> Option<SecretString> {
            self.gemini_key.clone()
        }

        fn groq_key(&self) -> Option<SecretString> {
            self.groq_key.clone()
        }

        fn openrouter_key(&self) -> Option<SecretString> {
            self.openrouter_key.clone()
        }
    }

    #[test]
    fn test_mock_provider_with_tokens() {
        let provider = MockTokenProvider {
            github_token: Some(SecretString::new("gh_token".to_string().into())),
            cerebras_key: Some(SecretString::new("cerebras_key".to_string().into())),
            gemini_key: Some(SecretString::new("gemini_key".to_string().into())),
            groq_key: Some(SecretString::new("groq_key".to_string().into())),
            openrouter_key: Some(SecretString::new("or_key".to_string().into())),
        };

        assert!(provider.github_token().is_some());
        assert!(provider.cerebras_key().is_some());
        assert!(provider.gemini_key().is_some());
        assert!(provider.groq_key().is_some());
        assert!(provider.openrouter_key().is_some());
    }

    #[test]
    fn test_mock_provider_without_tokens() {
        let provider = MockTokenProvider {
            github_token: None,
            cerebras_key: None,
            gemini_key: None,
            groq_key: None,
            openrouter_key: None,
        };

        assert!(provider.github_token().is_none());
        assert!(provider.cerebras_key().is_none());
        assert!(provider.gemini_key().is_none());
        assert!(provider.groq_key().is_none());
        assert!(provider.openrouter_key().is_none());
    }
}
