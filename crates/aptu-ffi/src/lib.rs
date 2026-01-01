// SPDX-License-Identifier: Apache-2.0

pub mod auth;
pub mod error;
pub mod keychain;
pub mod types;

use crate::error::AptuFfiError;
use crate::keychain::KeychainProviderRef;
use crate::types::{
    FfiAiModel, FfiApplyResult, FfiCuratedRepo, FfiIssueNode, FfiReleaseNotesResponse,
    FfiTokenStatus, FfiTriageResponse,
};
use tokio::runtime::Runtime;

lazy_static::lazy_static! {
    static ref RUNTIME: Runtime = Runtime::new().expect("Failed to create Tokio runtime");
}

#[uniffi::export]
pub fn list_curated_repos() -> Result<Vec<FfiCuratedRepo>, AptuFfiError> {
    RUNTIME.block_on(async {
        match aptu_core::list_curated_repos().await {
            Ok(repos) => Ok(repos.iter().map(FfiCuratedRepo::from).collect()),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
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
        let provider = auth::FfiTokenProvider::new(keychain);

        match aptu_core::fetch_issues(&provider, None, true).await {
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
        // Load configuration at FFI boundary
        let config = aptu_core::config::load_config().map_err(|e| AptuFfiError::InternalError {
            message: format!("Failed to load config: {e}"),
        })?;

        let provider = auth::FfiTokenProvider::new(keychain);

        let core_issue = aptu_core::ai::types::IssueDetails::builder()
            .owner(String::new())
            .repo(String::new())
            .number(issue.number)
            .title(issue.title)
            .body(issue.body)
            .labels(issue.labels)
            .comments(vec![])
            .url(issue.url)
            .build();

        match aptu_core::analyze_issue(&provider, &core_issue, &config.ai).await {
            Ok(ai_response) => Ok(FfiTriageResponse::from(ai_response.triage)),
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

/// Posts a PR review to GitHub.
///
/// This function wires the FFI layer to the core facade by:
/// 1. Creating an FfiTokenProvider from the iOS keychain
/// 2. Calling the core facade post_pr_review() function
/// 3. Returning the review ID on success
///
/// # Arguments
///
/// * `keychain` - iOS keychain provider for credential resolution
/// * `reference` - PR reference (URL, owner/repo#number, or number)
/// * `repo_context` - Optional repository context for bare numbers
/// * `body` - Review comment text
/// * `event_type` - Review event type: "COMMENT", "APPROVE", or "REQUEST_CHANGES"
///
/// # Returns
///
/// The review ID on success.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available in keychain
/// - PR cannot be parsed or found
/// - User lacks write access to the repository
/// - API call fails
#[uniffi::export]
pub fn post_pr_review(
    keychain: KeychainProviderRef,
    reference: String,
    repo_context: Option<String>,
    body: String,
    event_type: String,
) -> Result<u64, AptuFfiError> {
    RUNTIME.block_on(async {
        let provider = auth::FfiTokenProvider::new(keychain);

        // Parse event type string to ReviewEvent enum
        let event = match event_type.as_str() {
            "COMMENT" => aptu_core::ReviewEvent::Comment,
            "APPROVE" => aptu_core::ReviewEvent::Approve,
            "REQUEST_CHANGES" => aptu_core::ReviewEvent::RequestChanges,
            _ => {
                return Err(AptuFfiError::InternalError {
                    message: format!(
                        "Invalid event type: {}. Expected COMMENT, APPROVE, or REQUEST_CHANGES",
                        event_type
                    ),
                });
            }
        };

        match aptu_core::post_pr_review(
            &provider,
            &reference,
            repo_context.as_deref(),
            &body,
            event,
        )
        .await
        {
            Ok(review_id) => Ok(review_id),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
}

/// Add a custom repository.
///
/// Validates the repository via GitHub API and adds it to the custom repos file.
///
/// # Arguments
///
/// * `owner` - Repository owner
/// * `name` - Repository name
///
/// # Returns
///
/// The added repository details.
///
/// # Errors
///
/// Returns an error if:
/// - Repository cannot be found on GitHub
/// - Custom repos file cannot be read or written
#[uniffi::export]
pub fn add_custom_repo(owner: String, name: String) -> Result<FfiCuratedRepo, AptuFfiError> {
    RUNTIME.block_on(async {
        match aptu_core::add_custom_repo(&owner, &name).await {
            Ok(repo) => Ok(FfiCuratedRepo::from(&repo)),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
}

/// Remove a custom repository.
///
/// # Arguments
///
/// * `owner` - Repository owner
/// * `name` - Repository name
///
/// # Returns
///
/// True if the repository was removed, false if it was not found.
///
/// # Errors
///
/// Returns an error if the custom repos file cannot be read or written.
#[uniffi::export]
pub fn remove_custom_repo(owner: String, name: String) -> Result<bool, AptuFfiError> {
    match aptu_core::remove_custom_repo(&owner, &name) {
        Ok(removed) => Ok(removed),
        Err(e) => Err(AptuFfiError::InternalError {
            message: e.to_string(),
        }),
    }
}

/// List repositories with optional filtering.
///
/// # Arguments
///
/// * `filter_type` - Filter type: "all", "curated", or "custom"
///
/// # Returns
///
/// A vector of repositories matching the filter.
///
/// # Errors
///
/// Returns an error if repositories cannot be fetched.
#[uniffi::export]
pub fn list_repos(filter_type: String) -> Result<Vec<FfiCuratedRepo>, AptuFfiError> {
    RUNTIME.block_on(async {
        let filter = match filter_type.as_str() {
            "curated" => aptu_core::RepoFilter::Curated,
            "custom" => aptu_core::RepoFilter::Custom,
            _ => aptu_core::RepoFilter::All,
        };

        match aptu_core::list_repos(filter).await {
            Ok(repos) => Ok(repos.iter().map(FfiCuratedRepo::from).collect()),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
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

/// List all available AI providers with their metadata.
///
/// Returns the complete registry of providers that Aptu supports,
/// including their API endpoints, authentication requirements, and available models.
///
/// # Returns
///
/// A vector of provider names and their configurations.
#[uniffi::export]
pub fn list_providers() -> Vec<String> {
    aptu_core::ai::all_providers()
        .iter()
        .map(|p| p.name.to_string())
        .collect()
}

/// Fetch an issue for triage analysis.
///
/// Parses the issue reference, fetches issue details including labels, milestones,
/// and repository context (related issues, file tree).
///
/// # Arguments
///
/// * `keychain` - iOS keychain provider for credential resolution
/// * `reference` - Issue reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
///
/// # Returns
///
/// Issue details including title, body, labels, comments, and available labels/milestones.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available in keychain
/// - Issue reference cannot be parsed
/// - GitHub API call fails
#[uniffi::export]
pub fn fetch_issue_for_triage(
    keychain: KeychainProviderRef,
    reference: String,
    repo_context: Option<String>,
) -> Result<crate::types::FfiIssueDetails, AptuFfiError> {
    RUNTIME.block_on(async {
        let provider = auth::FfiTokenProvider::new(keychain);

        match aptu_core::fetch_issue_for_triage(&provider, &reference, repo_context.as_deref())
            .await
        {
            Ok(issue_details) => {
                let ffi_issue = crate::types::FfiIssueDetails {
                    number: issue_details.number,
                    title: issue_details.title,
                    body: issue_details.body,
                    labels: issue_details.labels,
                    url: issue_details.url,
                    author: issue_details.author.unwrap_or_default(),
                    created_at: issue_details.created_at.unwrap_or_default(),
                    updated_at: issue_details.updated_at.unwrap_or_default(),
                    comments_count: issue_details.comments.len() as u32,
                };
                Ok(ffi_issue)
            }
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
}

/// Post a triage comment to GitHub.
///
/// Renders the triage response as markdown and posts it as a comment on the issue.
///
/// # Arguments
///
/// * `keychain` - iOS keychain provider for credential resolution
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `number` - Issue number
/// * `triage` - Triage response to post
///
/// # Returns
///
/// The URL of the posted comment.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available in keychain
/// - GitHub API call fails
#[uniffi::export]
pub fn post_triage_comment(
    keychain: KeychainProviderRef,
    owner: String,
    repo: String,
    number: u64,
    triage: FfiTriageResponse,
) -> Result<String, AptuFfiError> {
    RUNTIME.block_on(async {
        let provider = auth::FfiTokenProvider::new(keychain);

        // Convert FfiTriageResponse back to core TriageResponse
        let core_triage: aptu_core::ai::types::TriageResponse = triage.into();

        // Create minimal IssueDetails for the facade call
        let issue_details = aptu_core::ai::types::IssueDetails::builder()
            .owner(owner)
            .repo(repo)
            .number(number)
            .title(String::new())
            .body(String::new())
            .labels(vec![])
            .comments(vec![])
            .url(String::new())
            .build();

        match aptu_core::post_triage_comment(&provider, &issue_details, &core_triage).await {
            Ok(comment_url) => Ok(comment_url),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
}

/// Apply AI-suggested labels and milestone to an issue.
///
/// Labels are applied additively: existing labels are preserved and AI-suggested labels
/// are merged in. Priority labels defer to existing human judgment.
///
/// # Arguments
///
/// * `keychain` - iOS keychain provider for credential resolution
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `number` - Issue number
/// * `current_labels` - Current labels on the issue
/// * `triage` - AI triage response with suggestions
///
/// # Returns
///
/// Result of applying labels and milestone.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available in keychain
/// - GitHub API call fails
#[uniffi::export]
pub fn apply_triage_labels(
    keychain: KeychainProviderRef,
    owner: String,
    repo: String,
    number: u64,
    current_labels: Vec<String>,
    triage: FfiTriageResponse,
) -> Result<FfiApplyResult, AptuFfiError> {
    RUNTIME.block_on(async {
        let provider = auth::FfiTokenProvider::new(keychain);

        // Convert FfiTriageResponse back to core TriageResponse
        let core_triage: aptu_core::ai::types::TriageResponse = triage.into();

        // Create minimal IssueDetails for the facade call
        let issue_details = aptu_core::ai::types::IssueDetails::builder()
            .owner(owner)
            .repo(repo)
            .number(number)
            .title(String::new())
            .body(String::new())
            .labels(current_labels)
            .comments(vec![])
            .url(String::new())
            .available_labels(vec![])
            .available_milestones(vec![])
            .build();

        match aptu_core::apply_triage_labels(&provider, &issue_details, &core_triage).await {
            Ok(result) => Ok(FfiApplyResult::from(result)),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
}

/// Generate AI-curated release notes from PRs between git tags.
///
/// This function wires the FFI layer to the core facade by:
/// 1. Creating an FfiTokenProvider from the iOS keychain
/// 2. Calling the core facade generate_release_notes() function
/// 3. Converting ReleaseNotesResponse to FfiReleaseNotesResponse using From trait
///
/// # Arguments
///
/// * `keychain` - iOS keychain provider for credential resolution
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `from_tag` - Starting git tag (optional, defaults to previous release)
/// * `to_tag` - Ending git tag (optional, defaults to HEAD)
///
/// # Returns
///
/// Structured release notes with theme, narrative, and categorized changes.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub or OpenRouter token is not available in keychain
/// - Git tags cannot be found or parsed
/// - AI API call fails
/// - Response parsing fails
#[uniffi::export]
pub fn generate_release_notes(
    keychain: KeychainProviderRef,
    owner: String,
    repo: String,
    from_tag: Option<String>,
    to_tag: Option<String>,
) -> Result<FfiReleaseNotesResponse, AptuFfiError> {
    RUNTIME.block_on(async {
        let provider = auth::FfiTokenProvider::new(keychain);

        match aptu_core::generate_release_notes(
            &provider,
            &owner,
            &repo,
            from_tag.as_deref(),
            to_tag.as_deref(),
        )
        .await
        {
            Ok(response) => Ok(FfiReleaseNotesResponse::from(response)),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
}

/// Post release notes to GitHub as a release.
///
/// Creates or updates a GitHub release with the provided release notes body.
///
/// # Arguments
///
/// * `keychain` - iOS keychain provider for credential resolution
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `tag` - Git tag for the release
/// * `body` - Release notes body (markdown)
///
/// # Returns
///
/// The URL of the created/updated release.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available in keychain
/// - User lacks write access to the repository
/// - GitHub API call fails
#[uniffi::export]
pub fn post_release_notes(
    keychain: KeychainProviderRef,
    owner: String,
    repo: String,
    tag: String,
    body: String,
) -> Result<String, AptuFfiError> {
    RUNTIME.block_on(async {
        let provider = auth::FfiTokenProvider::new(keychain);

        match aptu_core::post_release_notes(&provider, &owner, &repo, &tag, &body).await {
            Ok(url) => Ok(url),
            Err(e) => Err(AptuFfiError::InternalError {
                message: e.to_string(),
            }),
        }
    })
}

uniffi::setup_scaffolding!();
