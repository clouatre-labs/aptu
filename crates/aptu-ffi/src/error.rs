// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

#[derive(Error, Debug, uniffi::Error)]
pub enum AptuFfiError {
    #[error("Not authenticated - run auth first")]
    NotAuthenticated,

    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("API error: {message}")]
    ApiError { message: String },

    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    #[error("Keychain error: {message}")]
    KeychainError { message: String },

    #[error("Internal error: {message}")]
    InternalError { message: String },

    #[error("Model provider not found: {provider}")]
    ProviderNotFound { provider: String },

    #[error("Model cache error: {message}")]
    CacheError { message: String },
}

impl From<anyhow::Error> for AptuFfiError {
    fn from(err: anyhow::Error) -> Self {
        let message = err.to_string();
        AptuFfiError::InternalError { message }
    }
}

impl From<serde_json::Error> for AptuFfiError {
    fn from(err: serde_json::Error) -> Self {
        AptuFfiError::InternalError {
            message: format!("JSON error: {}", err),
        }
    }
}
