// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 AAIF

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use octocrab::{GraphqlResponse, Octocrab};
use serde_json::{Value, json};

fn serve(listener: TcpListener, body: String, graphql_error: bool) {
    thread::spawn(move || {
        for stream in listener.incoming().take(if graphql_error { 2 } else { 1 }) {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            let response_body = if graphql_error {
                if String::from_utf8_lossy(&request).starts_with("POST") {
                    r#"{"errors":[{"type":"NOT_FOUND","message":"not found","extensions":{"type":"NOT_FOUND"}}]}"#.to_owned()
                } else {
                    body.clone()
                }
            } else {
                body.clone()
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
}

fn client_with_server(body: &str) -> (Octocrab, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let uri = format!("http://{}", listener.local_addr().unwrap());
    let client = Octocrab::builder().base_uri(uri).unwrap().build().unwrap();
    serve(listener.try_clone().unwrap(), body.to_owned(), false);
    (client, listener)
}

fn client_with_graphql_error_then(body: &str) -> (Octocrab, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let uri = format!("http://{}", listener.local_addr().unwrap());
    let client = Octocrab::builder().base_uri(uri).unwrap().build().unwrap();
    serve(listener.try_clone().unwrap(), body.to_owned(), true);
    (client, listener)
}

#[test]
fn graphql_response_deserializes_unwrapped_data() {
    let response: GraphqlResponse<Value> = serde_json::from_value(json!({
        "data": {"viewer": {"login": "clouatre"}}
    }))
    .unwrap();
    match response {
        GraphqlResponse::Ok(ok) => {
            assert_eq!(ok.data["viewer"]["login"], "clouatre");
            assert!(ok.data.get("data").is_none());
        }
        GraphqlResponse::Err(_) => panic!("expected successful GraphQL response"),
    }
}

#[tokio::test]
async fn fetch_issue_not_found_falls_back_to_pr() {
    let body = r#"{"id":1,"number":42,"title":"Fix","state":"open","html_url":"https://github.com/owner/repo/pull/42","url":"https://api.github.com/repos/owner/repo/pulls/42","head":{"label":"owner:fix","ref":"fix","sha":"abc","repo":null,"user":null},"base":{"label":"owner:main","ref":"main","sha":"def","repo":null,"user":null}}"#;
    let (client, _listener) = client_with_graphql_error_then(body);
    let error =
        aptu_core::github::graphql::fetch_issue_with_repo_context(&client, "owner", "repo", 42)
            .await
            .expect_err("a pull request must be reported as a type mismatch");
    let mismatch = error.downcast_ref::<aptu_core::AptuError>();
    assert!(matches!(
        mismatch,
        Some(aptu_core::AptuError::TypeMismatch {
            actual: aptu_core::error::ResourceType::PullRequest,
            ..
        })
    ));
}

#[tokio::test]
async fn fetch_issues_uses_unwrapped_mock_data() {
    let body = r#"{"data":{"repo0":{"nameWithOwner":"owner/repo","issues":{"nodes":[{"number":7,"title":"test","createdAt":"2026-01-01T00:00:00Z","labels":{"nodes":[]},"url":"https://example.test/7"}]}}}}"#;
    let (client, _listener) = client_with_server(body);
    let results = aptu_core::github::graphql::fetch_issues(&client, &[("owner", "repo")])
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "owner/repo");
    assert_eq!(results[0].1.len(), 1);
}

#[tokio::test]
async fn resolve_tag_unwrapped_data_and_absent_target_return_none() {
    let body = r#"{"data":{"repository":{"ref":null}}}"#;
    let (client, _listener) = client_with_server(body);
    let result =
        aptu_core::github::graphql::resolve_tag_to_commit_sha(&client, "owner", "repo", "missing")
            .await
            .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn resolve_tag_unwrapped_data_and_present_target_return_sha() {
    let body = r#"{"data":{"repository":{"ref":{"target":{"oid":"abc123"}}}}}"#;
    let (client, _listener) = client_with_server(body);
    let result =
        aptu_core::github::graphql::resolve_tag_to_commit_sha(&client, "owner", "repo", "v1.0.0")
            .await
            .unwrap();
    assert_eq!(result, Some("abc123".to_owned()));
}

// Unauthenticated requests share GitHub's 60/hr rate limit across the whole
// runner IP and fail intermittently; authenticate so CI gets the higher
// per-token limit instead.
fn authenticated_live_client() -> Option<Octocrab> {
    let token = std::env::var("GITHUB_TOKEN").ok()?;
    Some(Octocrab::builder().personal_token(token).build().unwrap())
}

#[tokio::test]
#[ignore = "live GitHub API; run in CI graphql-contract job"]
async fn live_octocrab_graphql_returns_unwrapped_data() {
    let Some(client) = authenticated_live_client() else {
        eprintln!("skipping: GITHUB_TOKEN not set");
        return;
    };
    let value: Value = client
        .graphql(&json!({"query": "query { viewer { login } }"}))
        .await
        .unwrap();
    assert!(value.get("data").is_none());
    assert!(value.get("viewer").is_some());
}

#[tokio::test]
#[ignore = "live GitHub API; run in CI graphql-contract job"]
async fn live_fetch_issues_end_to_end() {
    let Some(client) = authenticated_live_client() else {
        eprintln!("skipping: GITHUB_TOKEN not set");
        return;
    };
    // fetch_issues only returns a repo when it has an open, unassigned
    // "good first issue"; that backlog changes independently of us, so this
    // only asserts the live call succeeds and any returned repo is the one
    // we asked for. The deterministic unwrap-regression coverage lives in
    // the mocked tests above.
    let results = aptu_core::github::graphql::fetch_issues(&client, &[("aaif-goose", "goose")])
        .await
        .unwrap();
    assert!(results.len() <= 1);
    if let Some((name, issues)) = results.first() {
        assert_eq!(name, "aaif-goose/goose");
        assert!(!issues.is_empty());
    }
}
