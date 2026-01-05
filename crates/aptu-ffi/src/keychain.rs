// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

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
        "com.block.aptu".to_string(),
        "github_token".to_string(),
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
    keychain.get_token("com.block.aptu".to_string(), "github_token".to_string())
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
    keychain.delete_token("com.block.aptu".to_string(), "github_token".to_string())
}
