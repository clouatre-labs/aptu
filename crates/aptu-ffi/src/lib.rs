pub mod auth;
pub mod error;
pub mod keychain;
pub mod types;

use crate::error::AptuFfiError;
use crate::keychain::KeychainProviderRef;
use crate::types::{FfiAiModel, FfiCuratedRepo, FfiIssueNode, FfiTokenStatus, FfiTriageResponse};
use aptu_core::auth::TokenProvider;
use secrecy::SecretString;
use tokio::runtime::Runtime;

lazy_static::lazy_static! {
    static ref RUNTIME: Runtime = Runtime::new().expect("Failed to create Tokio runtime");
}

/// FFI TokenProvider implementation that bridges iOS keychain to core TokenProvider trait.
struct FfiTokenProvider {
    keychain: KeychainProviderRef,
}

impl FfiTokenProvider {
    fn new(keychain: KeychainProviderRef) -> Self {
        Self { keychain }
    }
}

impl TokenProvider for FfiTokenProvider {
    fn github_token(&self) -> Option<SecretString> {
        match self
            .keychain
            .get_token("aptu".to_string(), "github".to_string())
        {
            Ok(Some(token)) => Some(SecretString::new(token.into())),
            _ => None,
        }
    }

    fn openrouter_key(&self) -> Option<SecretString> {
        match self
            .keychain
            .get_token("aptu".to_string(), "openrouter".to_string())
        {
            Ok(Some(key)) => Some(SecretString::new(key.into())),
            _ => None,
        }
    }
}

#[uniffi::export]
pub fn list_curated_repos() -> Vec<FfiCuratedRepo> {
    let repos = aptu_core::repos::list();
    repos.iter().map(FfiCuratedRepo::from).collect()
}

/// Fetch "good first issue" issues from all curated repositories.
///
/// This function wires the FFI layer to the core facade by:
/// 1. Creating an FfiTokenProvider from the iOS keychain
/// 2. Calling the core facade fetch_issues() function
/// 3. Converting IssueNode results to FfiIssueNode using From trait
///
/// # Arguments
///
/// * `keychain` - iOS keychain provider for credential resolution
///
/// # Returns
///
/// A vector of FfiIssueNode structs representing issues from curated repos.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available in keychain
/// - GitHub API call fails
/// - Response parsing fails
#[uniffi::export]
pub fn fetch_issues(keychain: KeychainProviderRef) -> Result<Vec<FfiIssueNode>, AptuFfiError> {
    RUNTIME.block_on(async {
        let provider = FfiTokenProvider::new(keychain);

        match aptu_core::fetch_issues(&provider, None).await {
            Ok(results) => {
                let mut ffi_issues = Vec::new();
                for (_repo_name, issues) in results {
                    for issue in issues {
                        ffi_issues.push(FfiIssueNode::from(issue));
                    }
                }
                Ok(ffi_issues)
            }
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
}

/// Analyze a GitHub issue and generate triage suggestions.
///
/// This function wires the FFI layer to the core facade by:
/// 1. Creating an FfiTokenProvider from the iOS keychain
/// 2. Calling the core facade analyze_issue() function
/// 3. Converting TriageResponse to FfiTriageResponse using From trait
///
/// # Arguments
///
/// * `keychain` - iOS keychain provider for credential resolution
/// * `issue` - Issue details to analyze
///
/// # Returns
///
/// Structured triage response with summary, labels, questions, and duplicates.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub or OpenRouter token is not available in keychain
/// - AI API call fails
/// - Response parsing fails
#[uniffi::export]
pub fn analyze_issue(
    keychain: KeychainProviderRef,
    issue: crate::types::FfiIssueDetails,
) -> Result<FfiTriageResponse, AptuFfiError> {
    RUNTIME.block_on(async {
        let provider = FfiTokenProvider::new(keychain);

        let core_issue = aptu_core::ai::types::IssueDetails {
            owner: String::new(),
            repo: String::new(),
            number: issue.number,
            title: issue.title,
            body: issue.body,
            labels: issue.labels,
            comments: vec![],
            url: issue.url,
            repo_context: vec![],
            repo_tree: vec![],
        };

        match aptu_core::analyze_issue(&provider, &core_issue).await {
            Ok(triage) => Ok(FfiTriageResponse::from(triage)),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
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

/// List all available AI models across all providers.
///
/// Returns the complete registry of models that Aptu supports,
/// including free and paid tiers from OpenRouter, Ollama, and MLX.
///
/// # Returns
///
/// A vector of FfiAiModel structs representing all available models.
#[uniffi::export]
pub fn list_available_models() -> Vec<FfiAiModel> {
    aptu_core::ai::models::AiModel::available_models()
        .into_iter()
        .map(FfiAiModel::from)
        .collect()
}

/// Get the default free AI model for new users.
///
/// Returns the recommended starting model for users without API keys.
/// This is the first free OpenRouter model in the registry.
///
/// # Returns
///
/// The default free model (Devstral 2).
#[uniffi::export]
pub fn get_default_model() -> FfiAiModel {
    FfiAiModel::from(aptu_core::ai::models::AiModel::default_free())
}

uniffi::setup_scaffolding!();
