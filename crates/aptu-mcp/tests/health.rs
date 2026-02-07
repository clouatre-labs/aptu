//! Integration tests for health check functionality.

use aptu_mcp::{AptuServer, CredentialStatus, HealthCheckResponse};

// ---------------------------------------------------------------------------
// Response Structure Tests
// ---------------------------------------------------------------------------

#[test]
fn health_check_response_all_valid() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Valid,
        ai_api_key: CredentialStatus::Valid,
    };
    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains(r#""github_token": "Valid""#));
    assert!(json.contains(r#""ai_api_key": "Valid""#));
}

#[test]
fn health_check_response_all_missing() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Missing,
        ai_api_key: CredentialStatus::Missing,
    };
    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains(r#""github_token": "Missing""#));
    assert!(json.contains(r#""ai_api_key": "Missing""#));
}

#[test]
fn health_check_response_mixed_status() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Valid,
        ai_api_key: CredentialStatus::Missing,
    };
    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains(r#""github_token": "Valid""#));
    assert!(json.contains(r#""ai_api_key": "Missing""#));
}

#[test]
fn health_check_response_github_invalid() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Invalid,
        ai_api_key: CredentialStatus::Valid,
    };
    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains(r#""github_token": "Invalid""#));
    assert!(json.contains(r#""ai_api_key": "Valid""#));
}

// ---------------------------------------------------------------------------
// Token Format Validation Tests
// ---------------------------------------------------------------------------

#[test]
fn github_token_format_valid_tokens() {
    // Test all valid GitHub token prefixes
    assert!(AptuServer::is_valid_github_token_format("ghp_1234567890"));
    assert!(AptuServer::is_valid_github_token_format("gho_1234567890"));
    assert!(AptuServer::is_valid_github_token_format("ghu_1234567890"));
    assert!(AptuServer::is_valid_github_token_format("ghs_1234567890"));
    assert!(AptuServer::is_valid_github_token_format("ghr_1234567890"));
    assert!(AptuServer::is_valid_github_token_format(
        "github_pat_1234567890"
    ));
}

#[test]
fn github_token_format_invalid_tokens() {
    // Invalid prefixes
    assert!(!AptuServer::is_valid_github_token_format("invalid_token"));
    assert!(!AptuServer::is_valid_github_token_format("abc_1234567890"));
    assert!(!AptuServer::is_valid_github_token_format("token_xyz"));

    // Empty and partial prefixes
    assert!(!AptuServer::is_valid_github_token_format(""));
    assert!(!AptuServer::is_valid_github_token_format("gh"));
    assert!(!AptuServer::is_valid_github_token_format("ghp"));
    assert!(!AptuServer::is_valid_github_token_format("github_"));
    assert!(!AptuServer::is_valid_github_token_format("github_pa"));
}
