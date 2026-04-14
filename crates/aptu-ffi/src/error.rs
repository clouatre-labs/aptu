// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

#[derive(Error, Debug, uniffi::Error)]
pub enum AptuFfiError {
    #[error("Not authenticated - run auth first")]
    NotAuthenticated,

    #[error("AI provider '{provider}' is not authenticated - set {env_var} environment variable")]
    AiProviderNotAuthenticated { provider: String, env_var: String },

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
}

pub(crate) fn ffi_error_from_anyhow(e: anyhow::Error) -> AptuFfiError {
    use aptu_core::error::AptuError;

    if let Some(core_err) = e.downcast_ref::<AptuError>() {
        match core_err {
            AptuError::NotAuthenticated => AptuFfiError::NotAuthenticated,
            AptuError::AiProviderNotAuthenticated { provider, env_var } => {
                AptuFfiError::AiProviderNotAuthenticated {
                    provider: provider.clone(),
                    env_var: env_var.clone(),
                }
            }
            AptuError::Network(_) => {
                let msg: String = e.to_string().chars().take(100).collect();
                AptuFfiError::NetworkError { message: msg }
            }
            AptuError::GitHub { message } => {
                let msg: String = message.chars().take(100).collect();
                AptuFfiError::ApiError { message: msg }
            }
            AptuError::AI { message, .. } => {
                let msg: String = message.chars().take(100).collect();
                AptuFfiError::ApiError { message: msg }
            }
            _ => {
                let msg: String = e.to_string().chars().take(100).collect();
                AptuFfiError::InternalError { message: msg }
            }
        }
    } else {
        let msg: String = e.to_string().chars().take(100).collect();
        AptuFfiError::InternalError { message: msg }
    }
}

impl From<anyhow::Error> for AptuFfiError {
    fn from(err: anyhow::Error) -> Self {
        ffi_error_from_anyhow(err)
    }
}

impl From<serde_json::Error> for AptuFfiError {
    fn from(err: serde_json::Error) -> Self {
        AptuFfiError::InternalError {
            message: format!("JSON error: {}", err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_error_not_authenticated_maps_correctly() {
        use aptu_core::error::AptuError;

        let e = anyhow::Error::new(AptuError::NotAuthenticated);
        let result = ffi_error_from_anyhow(e);
        assert!(matches!(result, AptuFfiError::NotAuthenticated));
    }

    #[test]
    fn test_ffi_error_ai_provider_not_authenticated() {
        use aptu_core::error::AptuError;

        let e = anyhow::Error::new(AptuError::AiProviderNotAuthenticated {
            provider: "openrouter".to_string(),
            env_var: "OPENROUTER_API_KEY".to_string(),
        });
        let result = ffi_error_from_anyhow(e);
        if let AptuFfiError::AiProviderNotAuthenticated { provider, env_var } = result {
            assert_eq!(provider, "openrouter");
            assert_eq!(env_var, "OPENROUTER_API_KEY");
        } else {
            panic!("expected AiProviderNotAuthenticated");
        }
    }

    #[test]
    fn test_ffi_error_unknown_truncates_message() {
        let long_msg = "x".repeat(200);
        let e = anyhow::anyhow!(long_msg);
        let result = ffi_error_from_anyhow(e);
        if let AptuFfiError::InternalError { message } = result {
            assert!(message.len() <= 100);
        } else {
            panic!("expected InternalError");
        }
    }
}
