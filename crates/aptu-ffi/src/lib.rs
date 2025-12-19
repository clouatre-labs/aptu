pub mod auth;
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
    let repos = aptu_core::repos::list();
    repos.iter().map(FfiCuratedRepo::from).collect()
}

/// Fetch issues from a GitHub repository.
///
/// TODO: This function currently returns hardcoded stub data. To wire it to the core:
/// 1. Design authentication bridge (see issue #48) to pass GitHub token from iOS keychain
/// 2. Call `aptu_core::github::graphql::fetch_issues(owner, repo, token)` with authenticated client
/// 3. Implement `From<IssueNode> for FfiIssueNode` in types.rs
/// 4. Map results using the From trait
///
/// Blocked by: feat(ffi): design authentication bridge for iOS
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

/// Analyze a GitHub issue and generate triage suggestions.
///
/// TODO: This function currently returns hardcoded stub data. To wire it to the core:
/// 1. Design authentication bridge (see issue #48) to pass GitHub token and AI API key from iOS
/// 2. Fetch issue details using `aptu_core::github::rest::get_issue(owner, repo, number, token)`
/// 3. Call `aptu_core::ai::openrouter::analyze_issue(issue_details, ai_config)` with authenticated client
/// 4. Implement `From<TriageResponse> for FfiTriageResponse` in types.rs
/// 5. Map results using the From trait
///
/// Blocked by: feat(ffi): design authentication bridge for iOS
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
