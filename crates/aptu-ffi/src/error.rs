// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

// Field names avoid `message` intentionally: UniFFI 0.29.x generates
// `val message: String` in Kotlin exception classes, which shadows
// `Throwable.message` and causes a compile error.  Using `msg` keeps
// the generated field name distinct from the inherited property.
#[derive(Error, Debug, uniffi::Error)]
pub enum AptuFfiError {
    #[error("Not authenticated - run auth first")]
    NotAuthenticated,

    #[error("AI provider '{provider}' is not authenticated - set {env_var} environment variable")]
    AiProviderNotAuthenticated { provider: String, env_var: String },

    #[error("Network error: {msg}")]
    NetworkError { msg: String },

    #[error("API error: {msg}")]
    ApiError { msg: String },

    #[error("Invalid input: {msg}")]
    InvalidInput { msg: String },

    #[error("Keychain error: {msg}")]
    KeychainError { msg: String },

    #[error("Internal error: {msg}")]
    InternalError { msg: String },
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
                AptuFfiError::NetworkError { msg }
            }
            AptuError::GitHub { message } => {
                let msg: String = message.chars().take(100).collect();
                AptuFfiError::ApiError { msg }
            }
            AptuError::AI { message, .. } => {
                let msg: String = message.chars().take(100).collect();
                AptuFfiError::ApiError { msg }
            }
            _ => {
                let msg: String = e.to_string().chars().take(100).collect();
                AptuFfiError::InternalError { msg }
            }
        }
    } else {
        let msg: String = e.to_string().chars().take(100).collect();
        AptuFfiError::InternalError { msg }
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
            msg: format!("JSON error: {}", err),
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
        if let AptuFfiError::InternalError { msg } = result {
            assert!(msg.len() <= 100);
        } else {
            panic!("expected InternalError");
        }
    }
}
