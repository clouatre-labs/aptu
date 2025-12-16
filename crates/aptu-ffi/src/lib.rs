pub mod error;
pub mod keychain;
pub mod types;

use crate::error::AptuFfiError;
use crate::keychain::KeychainProviderRef;
use crate::types::{FfiCuratedRepo, FfiIssueNode, FfiTokenStatus, FfiTriageResponse};
use tokio::runtime::Runtime;

lazy_static::lazy_static! {
    static ref RUNTIME: Runtime = Runtime::new().expect("Failed to create Tokio runtime");
}

#[uniffi::export]
pub fn list_curated_repos() -> Vec<FfiCuratedRepo> {
    RUNTIME.block_on(async {
        vec![
            FfiCuratedRepo {
                owner: "block".to_string(),
                name: "goose".to_string(),
                description: "AI-powered developer assistant".to_string(),
                language: "Rust".to_string(),
                open_issues_count: 42,
                last_activity: "2 days ago".to_string(),
            },
            FfiCuratedRepo {
                owner: "astral-sh".to_string(),
                name: "ruff".to_string(),
                description: "Fast Python linter and formatter".to_string(),
                language: "Rust".to_string(),
                open_issues_count: 128,
                last_activity: "1 day ago".to_string(),
            },
        ]
    })
}

#[uniffi::export]
pub fn fetch_issues(owner: String, repo: String) -> Result<Vec<FfiIssueNode>, AptuFfiError> {
    RUNTIME.block_on(async {
        if owner.is_empty() || repo.is_empty() {
            return Err(AptuFfiError::InvalidInput {
                message: "owner and repo cannot be empty".to_string(),
            });
        }

        Ok(vec![
            FfiIssueNode {
                number: 1234,
                title: "CLI crashes on empty config".to_string(),
                body: "When running aptu without a config file, it crashes".to_string(),
                labels: vec!["bug".to_string()],
                created_at: "2024-12-13T10:00:00Z".to_string(),
                updated_at: "2024-12-13T10:00:00Z".to_string(),
                url: format!("https://github.com/{}/{}/issues/1234", owner, repo),
            },
            FfiIssueNode {
                number: 1189,
                title: "Add --verbose flag".to_string(),
                body: "Would be helpful for debugging".to_string(),
                labels: vec!["enhancement".to_string()],
                created_at: "2024-12-12T10:00:00Z".to_string(),
                updated_at: "2024-12-12T10:00:00Z".to_string(),
                url: format!("https://github.com/{}/{}/issues/1189", owner, repo),
            },
        ])
    })
}

#[uniffi::export]
pub fn analyze_issue(issue_url: String) -> Result<FfiTriageResponse, AptuFfiError> {
    RUNTIME.block_on(async {
        if issue_url.is_empty() {
            return Err(AptuFfiError::InvalidInput {
                message: "issue_url cannot be empty".to_string(),
            });
        }

        Ok(FfiTriageResponse {
            summary: "This issue describes a crash when the config file is missing. The error handling should be improved to provide a helpful message.".to_string(),
            suggested_labels: vec![
                "bug".to_string(),
                "good first issue".to_string(),
            ],
            clarifying_questions: vec![
                "What is the exact error message?".to_string(),
                "Does this happen on all platforms?".to_string(),
            ],
            potential_duplicates: vec![],
        })
    })
}

#[uniffi::export]
pub fn check_auth_status(keychain: KeychainProviderRef) -> Result<FfiTokenStatus, AptuFfiError> {
    RUNTIME.block_on(async {
        match keychain.get_token("aptu".to_string(), "github".to_string()) {
            Ok(Some(_)) => Ok(FfiTokenStatus {
                is_authenticated: true,
                token_source: "keychain".to_string(),
                expires_at: None,
            }),
            Ok(None) => Ok(FfiTokenStatus {
                is_authenticated: false,
                token_source: "none".to_string(),
                expires_at: None,
            }),
            Err(e) => Err(e),
        }
    })
}

uniffi::setup_scaffolding!();
