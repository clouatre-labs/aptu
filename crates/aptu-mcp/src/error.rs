// SPDX-License-Identifier: Apache-2.0

//! Error conversion from aptu-core errors to MCP errors.

use aptu_core::error::AptuError;
use rmcp::model::{ErrorCode, ErrorData};

/// Convert `AptuError` into a typed MCP error based on error variant.
///
/// Maps error variants to appropriate MCP error codes:
/// - `TypeMismatch`, `ModelValidation`, `Config` -> `INVALID_PARAMS`
/// - `NotAuthenticated`, `AiProviderNotAuthenticated` -> `INVALID_REQUEST`
/// - All others -> `INTERNAL_ERROR`
pub fn aptu_error_to_mcp(err: &AptuError) -> ErrorData {
    let code = match err {
        AptuError::TypeMismatch { .. }
        | AptuError::ModelValidation { .. }
        | AptuError::Config { .. } => ErrorCode::INVALID_PARAMS,
        AptuError::NotAuthenticated | AptuError::AiProviderNotAuthenticated { .. } => {
            ErrorCode::INVALID_REQUEST
        }
        _ => ErrorCode::INTERNAL_ERROR,
    };

    match code {
        ErrorCode::INVALID_PARAMS => ErrorData::invalid_params(err.to_string(), None),
        ErrorCode::INVALID_REQUEST => ErrorData::invalid_request(err.to_string(), None),
        _ => ErrorData::internal_error(err.to_string(), None),
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
    fn converts_string_error_with_generic() {
        let err = generic_to_mcp_error("something went wrong");
        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("something went wrong"));
    }

    #[test]
    fn converts_io_error_with_generic() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = generic_to_mcp_error(io_err);
        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("file not found"));
    }

    #[test]
    fn aptu_type_mismatch_maps_to_invalid_params() {
        let err = AptuError::TypeMismatch {
            number: 123,
            expected: ResourceType::Issue,
            actual: ResourceType::PullRequest,
        };
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn aptu_model_validation_maps_to_invalid_params() {
        let err = AptuError::ModelValidation {
            model_id: "invalid-model".to_string(),
            suggestions: "gpt-4, claude-3".to_string(),
        };
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn aptu_config_maps_to_invalid_params() {
        let err = AptuError::Config {
            message: "missing api key".to_string(),
        };
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn aptu_not_authenticated_maps_to_invalid_request() {
        let err = AptuError::NotAuthenticated;
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INVALID_REQUEST);
    }

    #[test]
    fn aptu_ai_provider_not_authenticated_maps_to_invalid_request() {
        let err = AptuError::AiProviderNotAuthenticated {
            provider: "openrouter".to_string(),
            env_var: "OPENROUTER_API_KEY".to_string(),
        };
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INVALID_REQUEST);
    }

    #[test]
    fn aptu_github_error_maps_to_internal_error() {
        let err = AptuError::GitHub {
            message: "rate limited".to_string(),
        };
        let mcp_err = aptu_error_to_mcp(&err);
        assert_eq!(mcp_err.code, ErrorCode::INTERNAL_ERROR);
    }
}
