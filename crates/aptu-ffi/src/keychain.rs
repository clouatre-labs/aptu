// SPDX-License-Identifier: Apache-2.0

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
