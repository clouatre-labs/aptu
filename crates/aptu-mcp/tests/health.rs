// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the health check MCP tool.

use aptu_mcp::{AptuServer, CredentialStatus, HealthCheckResponse};

#[test]
fn credential_status_valid_variant() {
    let status = CredentialStatus::Valid;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"Valid\"");
}

#[test]
fn credential_status_missing_variant() {
    let status = CredentialStatus::Missing;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"Missing\"");
}

#[test]
fn credential_status_invalid_variant() {
    let status = CredentialStatus::Invalid;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"Invalid\"");
}

#[test]
fn health_check_response_all_valid() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Valid,
        ai_api_key: CredentialStatus::Valid,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains("\"github_token\": \"Valid\""));
    assert!(json.contains("\"ai_api_key\": \"Valid\""));
}

#[test]
fn health_check_response_all_missing() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Missing,
        ai_api_key: CredentialStatus::Missing,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains("\"github_token\": \"Missing\""));
    assert!(json.contains("\"ai_api_key\": \"Missing\""));
}

#[test]
fn health_check_response_mixed_status() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Valid,
        ai_api_key: CredentialStatus::Missing,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains("\"github_token\": \"Valid\""));
    assert!(json.contains("\"ai_api_key\": \"Missing\""));
}

#[test]
fn health_check_response_github_invalid() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Invalid,
        ai_api_key: CredentialStatus::Valid,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains("\"github_token\": \"Invalid\""));
    assert!(json.contains("\"ai_api_key\": \"Valid\""));
}

#[test]
fn health_check_response_ai_invalid() {
    let response = HealthCheckResponse {
        github_token: CredentialStatus::Valid,
        ai_api_key: CredentialStatus::Invalid,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains("\"github_token\": \"Valid\""));
    assert!(json.contains("\"ai_api_key\": \"Invalid\""));
}

#[test]
fn health_check_response_deserialize_valid() {
    let json = r#"{"github_token":"Valid","ai_api_key":"Valid"}"#;
    let response: HealthCheckResponse = serde_json::from_str(json).unwrap();
    assert!(matches!(response.github_token, CredentialStatus::Valid));
    assert!(matches!(response.ai_api_key, CredentialStatus::Valid));
}

#[test]
fn health_check_response_deserialize_missing() {
    let json = r#"{"github_token":"Missing","ai_api_key":"Missing"}"#;
    let response: HealthCheckResponse = serde_json::from_str(json).unwrap();
    assert!(matches!(response.github_token, CredentialStatus::Missing));
    assert!(matches!(response.ai_api_key, CredentialStatus::Missing));
}

#[test]
fn health_check_response_deserialize_invalid() {
    let json = r#"{"github_token":"Invalid","ai_api_key":"Invalid"}"#;
    let response: HealthCheckResponse = serde_json::from_str(json).unwrap();
    assert!(matches!(response.github_token, CredentialStatus::Invalid));
    assert!(matches!(response.ai_api_key, CredentialStatus::Invalid));
}

#[test]
fn health_check_response_deserialize_mixed() {
    let json = r#"{"github_token":"Valid","ai_api_key":"Missing"}"#;
    let response: HealthCheckResponse = serde_json::from_str(json).unwrap();
    assert!(matches!(response.github_token, CredentialStatus::Valid));
    assert!(matches!(response.ai_api_key, CredentialStatus::Missing));
}

#[test]
fn credential_status_copy_trait() {
    let status = CredentialStatus::Valid;
    let status_copy = status;
    assert!(matches!(status_copy, CredentialStatus::Valid));
}

#[test]
fn credential_status_clone_trait() {
    let status = CredentialStatus::Valid;
    let status_cloned = status.clone();
    assert!(matches!(status_cloned, CredentialStatus::Valid));
}

#[test]
fn health_check_response_json_schema() {
    let schema = schemars::schema_for!(HealthCheckResponse);
    let json = serde_json::to_value(&schema).unwrap();
    assert!(json.get("properties").is_some());
    let props = json.get("properties").unwrap();
    assert!(props.get("github_token").is_some());
    assert!(props.get("ai_api_key").is_some());
}

// ---------------------------------------------------------------------------
// Token Format Validation Tests
// ---------------------------------------------------------------------------

#[test]
fn github_token_format_valid_personal_access_token() {
    assert!(AptuServer::is_valid_github_token_format(
        "ghp_1234567890abcdefghijklmnopqrstuvwxyz"
    ));
}

#[test]
fn github_token_format_valid_oauth_token() {
    assert!(AptuServer::is_valid_github_token_format(
        "gho_1234567890abcdefghijklmnopqrstuvwxyz"
    ));
}

#[test]
fn github_token_format_valid_user_to_server() {
    assert!(AptuServer::is_valid_github_token_format(
        "ghu_1234567890abcdefghijklmnopqrstuvwxyz"
    ));
}

#[test]
fn github_token_format_valid_server_to_server() {
    assert!(AptuServer::is_valid_github_token_format(
        "ghs_1234567890abcdefghijklmnopqrstuvwxyz"
    ));
}

#[test]
fn github_token_format_valid_refresh_token() {
    assert!(AptuServer::is_valid_github_token_format(
        "ghr_1234567890abcdefghijklmnopqrstuvwxyz"
    ));
}

#[test]
fn github_token_format_valid_fine_grained() {
    assert!(AptuServer::is_valid_github_token_format(
        "github_pat_1234567890abcdefghijklmnopqrstuvwxyz"
    ));
}

#[test]
fn github_token_format_invalid_prefix() {
    assert!(!AptuServer::is_valid_github_token_format(
        "invalid_token_format"
    ));
    assert!(!AptuServer::is_valid_github_token_format("abc_1234567890"));
    assert!(!AptuServer::is_valid_github_token_format("token_xyz"));
}

#[test]
fn github_token_format_empty_string() {
    assert!(!AptuServer::is_valid_github_token_format(""));
}

#[test]
fn github_token_format_partial_prefix() {
    assert!(!AptuServer::is_valid_github_token_format("gh"));
    assert!(!AptuServer::is_valid_github_token_format("ghp"));
    assert!(!AptuServer::is_valid_github_token_format("github_"));
    assert!(!AptuServer::is_valid_github_token_format("github_pa"));
}
