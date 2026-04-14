// SPDX-License-Identifier: Apache-2.0

//! Error conversion from aptu-core errors to MCP errors.

use aptu_core::error::AptuError;
use rmcp::model::{ErrorCode, ErrorData};

/// Build the structured metadata that goes into the `data` field of an MCP error response.
///
/// Returns a JSON object with three machine-readable fields:
/// - `errorCategory`: stable uppercase string an orchestrator can switch on (e.g. `"RATE_LIMITED"`)
/// - `isRetryable`: `true` when the same request may succeed if retried without modification
/// - `suggestedAction`: human-readable hint (may include dynamic context such as retry delay)
fn error_meta(category: &'static str, retryable: bool, action: &str) -> serde_json::Value {
    serde_json::json!({
        "errorCategory": category,
        "isRetryable": retryable,
        "suggestedAction": action,
    })
}

/// Convert `AptuError` into a typed MCP error based on error variant.
///
/// Maps error variants to appropriate MCP error codes:
/// - `TypeMismatch`, `ModelValidation`, `Config` -> `INVALID_PARAMS`
/// - `NotAuthenticated`, `AiProviderNotAuthenticated` -> `INVALID_REQUEST`
/// - All others -> `INTERNAL_ERROR`
///
/// Additionally, each variant includes structured error metadata in the data field,
/// containing errorCategory, isRetryable, and suggestedAction.
#[allow(clippy::too_many_lines)]
pub fn aptu_error_to_mcp(err: &AptuError) -> ErrorData {
    let code = match err {
        AptuError::TypeMismatch { .. }
        | AptuError::ModelValidation { .. }
        | AptuError::Config { .. }
        | AptuError::SecurityScan { .. } => ErrorCode::INVALID_PARAMS,
        AptuError::NotAuthenticated | AptuError::AiProviderNotAuthenticated { .. } => {
            ErrorCode::INVALID_REQUEST
        }
        _ => ErrorCode::INTERNAL_ERROR,
    };

    let message = err.to_string();
    let data = match err {
        AptuError::GitHub { .. } => Some(error_meta(
            "GITHUB_ERROR",
            false,
            "Check GitHub API status or verify the repository/issue reference",
        )),
        AptuError::AI { .. } => Some(error_meta("AI_ERROR", false, "Check AI provider status")),
        AptuError::NotAuthenticated => Some(error_meta(
            "NOT_AUTHENTICATED",
            false,
            "Run aptu auth login to authenticate",
        )),
        AptuError::AiProviderNotAuthenticated { provider, env_var } => Some(error_meta(
            "AI_NOT_AUTHENTICATED",
            false,
            &format!("Set the {env_var} environment variable for {provider}"),
        )),
        AptuError::RateLimited {
            provider,
            retry_after,
        } => Some(error_meta(
            "RATE_LIMITED",
            true,
            &format!("Retry after {retry_after} seconds (provider: {provider})"),
        )),
        AptuError::TruncatedResponse { provider } => Some(error_meta(
            "TRUNCATED_RESPONSE",
            true,
            &format!("Retry with a longer max_tokens limit (provider: {provider})"),
        )),
        AptuError::Config { .. } => Some(error_meta(
            "CONFIG_ERROR",
            false,
            "Fix the configuration and retry",
        )),
        AptuError::InvalidAIResponse(_) => Some(error_meta(
            "INVALID_AI_RESPONSE",
            true,
            "Retry with a different model or prompt",
        )),
        AptuError::Network(_) => Some(error_meta(
            "NETWORK_ERROR",
            true,
            "Check network connectivity and retry",
        )),
        AptuError::CircuitOpen => Some(error_meta(
            "CIRCUIT_OPEN",
            true,
            "Wait for the circuit breaker to reset and retry",
        )),
        AptuError::TypeMismatch { .. } => Some(error_meta(
            "TYPE_MISMATCH",
            false,
            "Use the correct resource type",
        )),
        AptuError::ModelRegistry { .. } => Some(error_meta(
            "MODEL_REGISTRY_ERROR",
            false,
            "Check model configuration",
        )),
        AptuError::ModelValidation {
            model_id,
            suggestions,
        } => Some(error_meta(
            "MODEL_VALIDATION_ERROR",
            false,
            &if suggestions.is_empty() {
                format!("Invalid model ID: {model_id}")
            } else {
                format!("Invalid model ID: {model_id}. Suggestions: {suggestions}")
            },
        )),
        AptuError::SecurityScan { .. } => Some(error_meta(
            "SECURITY_SCAN_ERROR",
            false,
            "Prompt injection patterns detected; operation blocked for security",
        )),
        #[cfg(feature = "keyring")]
        AptuError::Keyring(_) => Some(error_meta(
            "KEYRING_ERROR",
            false,
            "Check system keyring configuration",
        )),
        #[allow(unreachable_patterns)]
        _ => Some(error_meta(
            "INTERNAL_ERROR",
            false,
            "An unexpected error occurred",
        )),
    };

    match code {
        ErrorCode::INVALID_PARAMS => ErrorData::invalid_params(message, data),
        ErrorCode::INVALID_REQUEST => ErrorData::invalid_request(message, data),
        _ => ErrorData::internal_error(message, data),
    }
}

/// Convert any error implementing Display into an MCP internal error.
pub fn generic_to_mcp_error<E: std::fmt::Display>(err: E) -> ErrorData {
    ErrorData::internal_error(err.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aptu_core::error::ResourceType;
    use rmcp::model::ErrorCode;

    #[test]
    fn generic_error_produces_internal_error_with_no_data() {
        let err = generic_to_mcp_error("something went wrong");
        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("something went wrong"));
        assert!(err.data.is_none());
    }

    #[test]
    fn type_mismatch_maps_to_invalid_params_and_is_not_retryable() {
        let err = AptuError::TypeMismatch {
            number: 123,
            expected: ResourceType::Issue,
            actual: ResourceType::PullRequest,
        };
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INVALID_PARAMS);
        let data = mcp_err.data.unwrap();
        assert_eq!(data["errorCategory"], "TYPE_MISMATCH");
        assert_eq!(data["isRetryable"], false);
    }

    #[test]
    fn not_authenticated_maps_to_invalid_request_and_is_not_retryable() {
        let err = AptuError::NotAuthenticated;
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INVALID_REQUEST);
        let data = mcp_err.data.unwrap();
        assert_eq!(data["errorCategory"], "NOT_AUTHENTICATED");
        assert_eq!(data["isRetryable"], false);
    }

    #[test]
    fn rate_limited_is_retryable_and_includes_retry_after() {
        let err = AptuError::RateLimited {
            provider: "openrouter".to_string(),
            retry_after: 60,
        };
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INTERNAL_ERROR);
        let data = mcp_err.data.unwrap();
        assert_eq!(data["errorCategory"], "RATE_LIMITED");
        assert_eq!(data["isRetryable"], true);
        assert!(
            data["suggestedAction"]
                .as_str()
                .unwrap()
                .contains("60 seconds")
        );
    }

    #[test]
    fn circuit_open_is_retryable() {
        let err = AptuError::CircuitOpen;
        let data = aptu_error_to_mcp(&err).data.unwrap();
        assert_eq!(data["errorCategory"], "CIRCUIT_OPEN");
        assert_eq!(data["isRetryable"], true);
    }

    #[test]
    fn truncated_response_is_retryable_and_includes_provider() {
        let err = AptuError::TruncatedResponse {
            provider: "ollama".to_string(),
        };
        let data = aptu_error_to_mcp(&err).data.unwrap();
        assert_eq!(data["errorCategory"], "TRUNCATED_RESPONSE");
        assert_eq!(data["isRetryable"], true);
        assert!(data["suggestedAction"].as_str().unwrap().contains("ollama"));
    }

    #[test]
    fn model_validation_empty_suggestions_produces_clean_action() {
        let err = AptuError::ModelValidation {
            model_id: "bad-model".to_string(),
            suggestions: String::new(),
        };
        let data = aptu_error_to_mcp(&err).data.unwrap();
        assert_eq!(data["errorCategory"], "MODEL_VALIDATION_ERROR");
        assert_eq!(data["isRetryable"], false);
        assert!(
            data["suggestedAction"]
                .as_str()
                .unwrap()
                .contains("bad-model")
        );
        assert!(
            !data["suggestedAction"]
                .as_str()
                .unwrap()
                .contains("Suggestions")
        );
    }
}
