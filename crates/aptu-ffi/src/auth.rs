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
/// Resolves GitHub and AI provider credentials from the iOS keychain
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

    fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
        match self
            .keychain
            .get_token("aptu".to_string(), provider.to_string())
        {
            Ok(Some(key)) => {
                debug!("Retrieved {} API key from iOS keychain", provider);
                Some(SecretString::new(key.into()))
            }
            Ok(None) => {
                debug!("No {} API key found in iOS keychain", provider);
                None
            }
            Err(e) => {
                debug!(
                    error = ?e,
                    "Failed to retrieve {} API key from keychain",
                    provider
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AptuFfiError;
    use aptu_core::ai::registry::all_providers;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// Mock keychain for testing.
    struct MockKeychain {
        tokens: HashMap<String, String>,
    }

    impl crate::keychain::KeychainProvider for MockKeychain {
        fn get_token(
            &self,
            service: String,
            account: String,
        ) -> Result<Option<String>, AptuFfiError> {
            if service == "aptu" {
                Ok(self.tokens.get(account.as_str()).cloned())
            } else {
                Ok(None)
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
        let mut tokens = HashMap::new();
        tokens.insert("github".to_string(), "gh_test_token".to_string());

        let keychain = Arc::new(MockKeychain { tokens });
        let provider = FfiTokenProvider::new(keychain);

        assert!(provider.github_token().is_some());
        for provider_config in all_providers() {
            assert!(
                provider.ai_api_key(provider_config.name).is_none(),
                "Expected no key for provider: {}",
                provider_config.name
            );
        }
    }

    #[test]
    fn test_ffi_token_provider_with_all_tokens() {
        let mut tokens = HashMap::new();
        tokens.insert("github".to_string(), "gh_test_token".to_string());
        for provider_config in all_providers() {
            tokens.insert(
                provider_config.name.to_string(),
                format!("{}_test_key", provider_config.name),
            );
        }

        let keychain = Arc::new(MockKeychain { tokens });
        let provider = FfiTokenProvider::new(keychain);

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
    fn test_ffi_token_provider_without_tokens() {
        let keychain = Arc::new(MockKeychain {
            tokens: HashMap::new(),
        });

        let provider = FfiTokenProvider::new(keychain);

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
