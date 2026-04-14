// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 Aptu Contributors

use crate::error::AptuFfiError;
use std::sync::Arc;

#[uniffi::export(with_foreign)]
pub trait KeychainProvider: Send + Sync {
    fn get_token(&self, service: String, account: String) -> Result<Option<String>, AptuFfiError>;

    fn set_token(
        &self,
        service: String,
        account: String,
        token: String,
    ) -> Result<(), AptuFfiError>;

    fn delete_token(&self, service: String, account: String) -> Result<(), AptuFfiError>;
}

pub type KeychainProviderRef = Arc<dyn KeychainProvider>;

const KEYCHAIN_SERVICE: &str = "aptu";
const KEYCHAIN_ACCOUNT: &str = "github";

/// Store a GitHub OAuth token in the system keychain
///
/// # Arguments
///
/// * `token` - The GitHub OAuth access token to store
/// * `keychain` - The keychain provider implementation
///
/// # Returns
///
/// Returns `Ok(())` if the token was successfully stored, or an error if the operation failed.
///
/// # Example
///
/// ```ignore
/// let result = store_github_token("ghp_xxxx".to_string(), keychain_provider)?;
/// ```
#[uniffi::export]
pub fn store_github_token(
    token: String,
    keychain: KeychainProviderRef,
) -> Result<(), AptuFfiError> {
    keychain.set_token(
        KEYCHAIN_SERVICE.to_string(),
        KEYCHAIN_ACCOUNT.to_string(),
        token,
    )
}

/// Retrieve a GitHub OAuth token from the system keychain
///
/// # Arguments
///
/// * `keychain` - The keychain provider implementation
///
/// # Returns
///
/// Returns `Ok(Some(token))` if a token is found, `Ok(None)` if no token exists,
/// or an error if the operation failed.
#[uniffi::export]
pub fn get_github_token(keychain: KeychainProviderRef) -> Result<Option<String>, AptuFfiError> {
    keychain.get_token(KEYCHAIN_SERVICE.to_string(), KEYCHAIN_ACCOUNT.to_string())
}

/// Delete a GitHub OAuth token from the system keychain
///
/// # Arguments
///
/// * `keychain` - The keychain provider implementation
///
/// # Returns
///
/// Returns `Ok(())` if the token was successfully deleted, or an error if the operation failed.
#[uniffi::export]
pub fn delete_github_token(keychain: KeychainProviderRef) -> Result<(), AptuFfiError> {
    keychain.delete_token(KEYCHAIN_SERVICE.to_string(), KEYCHAIN_ACCOUNT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing
    struct MockKeychain {
        tokens: std::sync::Mutex<std::collections::HashMap<(String, String), String>>,
    }

    impl KeychainProvider for MockKeychain {
        fn get_token(
            &self,
            service: String,
            account: String,
        ) -> Result<Option<String>, AptuFfiError> {
            Ok(self
                .tokens
                .lock()
                .expect("lock poisoned")
                .get(&(service, account))
                .cloned())
        }

        fn set_token(
            &self,
            service: String,
            account: String,
            token: String,
        ) -> Result<(), AptuFfiError> {
            self.tokens
                .lock()
                .expect("lock poisoned")
                .insert((service, account), token);
            Ok(())
        }

        fn delete_token(&self, service: String, account: String) -> Result<(), AptuFfiError> {
            self.tokens
                .lock()
                .expect("lock poisoned")
                .remove(&(service, account));
            Ok(())
        }
    }

    #[test]
    fn test_store_and_retrieve_github_token() {
        let keychain = Arc::new(MockKeychain {
            tokens: std::sync::Mutex::new(std::collections::HashMap::new()),
        });

        let token = "ghp_test123456789".to_string();

        // Store token
        assert!(store_github_token(token.clone(), keychain.clone()).is_ok());

        // Retrieve token
        let retrieved = get_github_token(keychain.clone());
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), Some(token));
    }

    #[test]
    fn test_delete_github_token() {
        let keychain = Arc::new(MockKeychain {
            tokens: std::sync::Mutex::new(std::collections::HashMap::new()),
        });

        let token = "ghp_test123456789".to_string();

        // Store token
        assert!(store_github_token(token, keychain.clone()).is_ok());

        // Verify it exists
        let retrieved = get_github_token(keychain.clone());
        assert_eq!(retrieved.unwrap(), Some("ghp_test123456789".to_string()));

        // Delete token
        assert!(delete_github_token(keychain.clone()).is_ok());

        // Verify it's gone
        let retrieved = get_github_token(keychain.clone());
        assert_eq!(retrieved.unwrap(), None);
    }
}
