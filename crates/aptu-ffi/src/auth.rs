// SPDX-License-Identifier: Apache-2.0

//! FFI implementation of `TokenProvider` using iOS keychain.
//!
//! This module provides the FFI's credential resolution strategy,
//! wrapping the `KeychainProvider` from iOS to implement `TokenProvider`.

use crate::keychain::KeychainProviderRef;
use aptu_core::TokenProvider;
use secrecy::SecretString;
use tracing::debug;

/// FFI token provider wrapping iOS keychain.
///
/// Resolves GitHub and `OpenRouter` credentials from the iOS keychain
/// via the `KeychainProvider` interface.
pub struct FfiTokenProvider {
    keychain: KeychainProviderRef,
}

impl FfiTokenProvider {
    /// Creates a new FFI token provider with the given keychain.
    pub fn new(keychain: KeychainProviderRef) -> Self {
        Self { keychain }
    }
}

impl TokenProvider for FfiTokenProvider {
    fn github_token(&self) -> Option<SecretString> {
        match self
            .keychain
            .get_token("aptu".to_string(), "github".to_string())
        {
            Ok(Some(token)) => {
                debug!("Retrieved GitHub token from iOS keychain");
                Some(SecretString::new(token.into()))
            }
            Ok(None) => {
                debug!("No GitHub token found in iOS keychain");
                None
            }
            Err(e) => {
                debug!(error = ?e, "Failed to retrieve GitHub token from keychain");
                None
            }
        }
    }

    fn openrouter_key(&self) -> Option<SecretString> {
        match self
            .keychain
            .get_token("aptu".to_string(), "openrouter".to_string())
        {
            Ok(Some(key)) => {
                debug!("Retrieved OpenRouter API key from iOS keychain");
                Some(SecretString::new(key.into()))
            }
            Ok(None) => {
                debug!("No OpenRouter API key found in iOS keychain");
                None
            }
            Err(e) => {
                debug!(error = ?e, "Failed to retrieve OpenRouter API key from keychain");
                None
            }
        }
    }

    fn gemini_key(&self) -> Option<SecretString> {
        match self
            .keychain
            .get_token("aptu".to_string(), "gemini".to_string())
        {
            Ok(Some(key)) => {
                debug!("Retrieved Gemini API key from iOS keychain");
                Some(SecretString::new(key.into()))
            }
            Ok(None) => {
                debug!("No Gemini API key found in iOS keychain");
                None
            }
            Err(e) => {
                debug!(error = ?e, "Failed to retrieve Gemini API key from keychain");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AptuFfiError;
    use std::sync::Arc;

    /// Mock keychain for testing.
    struct MockKeychain {
        github_token: Option<String>,
        openrouter_key: Option<String>,
    }

    impl crate::keychain::KeychainProvider for MockKeychain {
        fn get_token(
            &self,
            service: String,
            account: String,
        ) -> Result<Option<String>, AptuFfiError> {
            match (service.as_str(), account.as_str()) {
                ("aptu", "github") => Ok(self.github_token.clone()),
                ("aptu", "openrouter") => Ok(self.openrouter_key.clone()),
                _ => Ok(None),
            }
        }

        fn set_token(
            &self,
            _service: String,
            _account: String,
            _token: String,
        ) -> Result<(), AptuFfiError> {
            Ok(())
        }

        fn delete_token(&self, _service: String, _account: String) -> Result<(), AptuFfiError> {
            Ok(())
        }
    }

    #[test]
    fn test_ffi_token_provider_with_github_token() {
        let keychain = Arc::new(MockKeychain {
            github_token: Some("gh_test_token".to_string()),
            openrouter_key: None,
        });

        let provider = FfiTokenProvider::new(keychain);
        assert!(provider.github_token().is_some());
        assert!(provider.openrouter_key().is_none());
    }

    #[test]
    fn test_ffi_token_provider_with_both_tokens() {
        let keychain = Arc::new(MockKeychain {
            github_token: Some("gh_test_token".to_string()),
            openrouter_key: Some("or_test_key".to_string()),
        });

        let provider = FfiTokenProvider::new(keychain);
        assert!(provider.github_token().is_some());
        assert!(provider.openrouter_key().is_some());
    }

    #[test]
    fn test_ffi_token_provider_without_tokens() {
        let keychain = Arc::new(MockKeychain {
            github_token: None,
            openrouter_key: None,
        });

        let provider = FfiTokenProvider::new(keychain);
        assert!(provider.github_token().is_none());
        assert!(provider.openrouter_key().is_none());
    }
}
