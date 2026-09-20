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

#[test]
fn issue_comment_node_null_author_deserializes_to_ghost() {
    let node: aptu_core::github::graphql::IssueCommentNode = serde_json::from_value(json!({
        "id": "1",
        "author": null,
        "body": "deleted user"
    }))
    .expect("a comment with a deleted (null) author must deserialize");
    let comment = aptu_core::ai::types::IssueComment::from(node);
    assert_eq!(comment.author, "ghost");
}

#[test]
fn issue_comment_node_present_author_keeps_login() {
    let node: aptu_core::github::graphql::IssueCommentNode = serde_json::from_value(json!({
        "id": "2",
        "author": {"login": "octocat"},
        "body": "hello"
    }))
    .unwrap();
    let comment = aptu_core::ai::types::IssueComment::from(node);
    assert_eq!(comment.author, "octocat");
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

#[tokio::test]
async fn fetch_repo_viewer_permission_reads_unwrapped_data() {
    // The wire body uses the full GraphQL envelope; octocrab unwraps `data`,
    // so fetch_repo_viewer_permission must read repository.viewerPermission
    // from the unwrapped response (same contract as fetch_issues).
    let body = r#"{"data":{"repository":{"viewerPermission":"READ"}}}"#;
    let (client, _listener) = client_with_server(body);
    let perm = aptu_core::github::graphql::fetch_repo_viewer_permission(&client, "owner", "repo")
        .await
        .unwrap();
    assert_eq!(
        perm,
        Some(aptu_core::github::graphql::ViewerPermission::Read)
    );
}

fn serve_forbidden_once(listener: std::net::TcpListener) {
    thread::spawn(move || {
        for stream in listener.incoming().take(1) {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            let body = r#"{"message":"Must have write access"}"#;
            let response = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
}

#[tokio::test]
async fn label_write_403_maps_to_permission_denied() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let uri = format!("http://{}", listener.local_addr().unwrap());
    let client = Octocrab::builder().base_uri(uri).unwrap().build().unwrap();
    serve_forbidden_once(listener);

    let err = aptu_core::github::issues::apply_labels_to_number(
        &client,
        "owner",
        "repo",
        1,
        &["bug".to_string()],
    )
    .await
    .expect_err("a 403 write must fail");

    let mapped = aptu_core::error::aptu_error_from_anyhow(err);
    assert!(matches!(
        mapped,
        aptu_core::AptuError::PermissionDenied { .. }
    ));
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

#[tokio::test]
async fn fetch_issue_with_repo_context_deserializes_string_comment_id() {
    let body = r#"{"data":{"issue":{"issue":{"number":42,"title":"Fix","body":"Body","url":"https://github.com/owner/repo/issues/42","author":{"login":"someone"},"createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-02T00:00:00Z","labels":{"nodes":[]},"comments":{"totalCount":1,"nodes":[{"id":"IC_kwDOAbc123","author":{"login":"aptu-bot"},"body":"Generated by Aptu"}]}}},"repository":{"nameWithOwner":"owner/repo","labels":{"nodes":[]},"milestones":{"nodes":[]},"primaryLanguage":null,"viewerPermission":"WRITE"}}}"#;
    let (client, _listener) = client_with_server(body);
    let (issue, _repo) =
        aptu_core::github::graphql::fetch_issue_with_repo_context(&client, "owner", "repo", 42)
            .await
            .unwrap();
    let comment = &issue.comments.nodes[0];
    assert_eq!(comment.id, "IC_kwDOAbc123");
    let converted: aptu_core::IssueComment = comment.clone().into();
    assert_eq!(converted.id, "IC_kwDOAbc123");
}
