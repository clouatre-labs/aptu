// SPDX-License-Identifier: Apache-2.0

//! Triage an issue with AI assistance command.
//!
//! Fetches a GitHub issue, analyzes it with AI, and optionally posts
//! a comment to GitHub. Split into `fetch()` and `analyze()` for proper
//! confirmation flow (render issue before asking).

use anyhow::{Context, Result};
use aptu_core::ai::AiResponse;
use aptu_core::error::AptuError;
use aptu_core::github::{auth, issues};
use aptu_core::{IssueDetails, TriageResponse, history::AiStats};
use tracing::{debug, info, instrument};

use crate::output::render_triage_markdown;

/// Intermediate result from analysis (before posting decision).
pub struct AnalyzeResult {
    /// Issue details (title, body, labels, comments).
    pub issue_details: IssueDetails,
    /// AI triage analysis.
    pub triage: TriageResponse,
    /// AI usage statistics.
    #[allow(dead_code)] // Used for future features (history tracking)
    pub ai_stats: AiStats,
}

/// Fetch an issue from GitHub.
///
/// Parses the issue reference, checks authentication, and fetches issue details.
/// Also searches for related issues to provide context to the AI.
/// Does not perform AI analysis.
///
/// # Arguments
///
/// * `reference` - Issue reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
#[instrument(skip_all, fields(reference = %reference))]
pub async fn fetch(reference: &str, repo_context: Option<&str>) -> Result<IssueDetails> {
    // Check authentication
    if !auth::is_authenticated() {
        return Err(AptuError::NotAuthenticated.into());
    }

    // Parse the issue reference
    let (owner, repo, number) = issues::parse_issue_reference(reference, repo_context)?;

    // Create authenticated client
    let client = auth::create_client().context("Failed to create GitHub client")?;

    // Fetch issue details
    let mut issue_details =
        issues::fetch_issue_with_comments(&client, &owner, &repo, number).await?;

    // Search for related issues to provide context to AI
    match issues::search_related_issues(&client, &owner, &repo, &issue_details.title).await {
        Ok(related) => {
            issue_details.repo_context = related;
            debug!(
                related_count = issue_details.repo_context.len(),
                "Found related issues"
            );
        }
        Err(e) => {
            // Log but don't fail - related issues are optional context
            debug!(error = %e, "Failed to search for related issues, continuing without context");
        }
    }

    // Fetch repository tree for implementation context
    // First, get the repository metadata to find the primary language
    match client.repos(&owner, &repo).get().await {
        Ok(repo_info) => {
            let language = repo_info
                .language
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            match issues::fetch_repo_tree(&client, &owner, &repo, language).await {
                Ok(tree) => {
                    issue_details.repo_tree = tree;
                    debug!(
                        tree_count = issue_details.repo_tree.len(),
                        "Fetched repository tree"
                    );
                }
                Err(e) => {
                    // Log but don't fail - repo tree is optional context
                    debug!(error = %e, "Failed to fetch repository tree, continuing without context");
                }
            }
        }
        Err(e) => {
            // Log but don't fail - repo tree is optional context
            debug!(error = %e, "Failed to fetch repository metadata, continuing without tree context");
        }
    }

    debug!(issue_number = number, "Issue fetched successfully");
    Ok(issue_details)
}

/// Analyze an issue with AI assistance.
///
/// Takes fetched issue details and runs AI analysis via the facade layer.
/// Returns both triage response and AI usage statistics.
/// Does not post anything.
///
/// # Arguments
///
/// * `issue_details` - Fetched issue details from `fetch()`
#[instrument(skip_all, fields(issue_number = issue_details.number))]
pub async fn analyze(issue_details: &IssueDetails) -> Result<AiResponse> {
    // Create CLI token provider
    let provider = crate::provider::CliTokenProvider;

    // Call facade for analysis
    let ai_response = aptu_core::analyze_issue(&provider, issue_details).await?;

    debug!("Issue analyzed successfully");
    Ok(ai_response)
}

/// Post a triage comment to GitHub.
#[instrument(skip_all, fields(issue_number = analyze_result.issue_details.number))]
pub async fn post(analyze_result: &AnalyzeResult) -> Result<String> {
    // Create authenticated client
    let client = auth::create_client().context("Failed to create GitHub client")?;

    // Post the comment
    let comment_body = render_triage_markdown(&analyze_result.triage);
    let comment_url = issues::post_comment(
        &client,
        &analyze_result.issue_details.owner,
        &analyze_result.issue_details.repo,
        analyze_result.issue_details.number,
        &comment_body,
    )
    .await?;

    info!(comment_url = %comment_url, "Triage comment posted");
    debug!(
        "Triage complete for issue #{}",
        analyze_result.issue_details.number
    );

    Ok(comment_url)
}
