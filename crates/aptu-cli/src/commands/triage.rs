// SPDX-License-Identifier: Apache-2.0

//! Triage an issue with AI assistance command.
//!
//! Fetches a GitHub issue, analyzes it with AI, and optionally posts
//! a comment to GitHub. Split into `fetch()` and `analyze()` for proper
//! confirmation flow (render issue before asking).

use anyhow::{Context, Result};
use aptu_core::ai::AiResponse;
use aptu_core::ai::types::{IssueComment, RepoLabel, RepoMilestone};
use aptu_core::error::AptuError;
use aptu_core::github::{auth, graphql, issues};
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

    // Fetch issue with repository context (labels, milestones) in a single GraphQL call
    let (issue_node, repo_data) =
        graphql::fetch_issue_with_repo_context(&client, &owner, &repo, number).await?;

    // Convert GraphQL response to IssueDetails
    let labels: Vec<String> = issue_node
        .labels
        .nodes
        .iter()
        .map(|l| l.name.clone())
        .collect();

    let comments: Vec<IssueComment> = issue_node
        .comments
        .nodes
        .iter()
        .map(|c| c.clone().into())
        .collect();

    // Convert repository labels to our type
    let available_labels: Vec<RepoLabel> = repo_data
        .labels
        .nodes
        .iter()
        .map(|l| l.clone().into())
        .collect();

    // Convert repository milestones to our type
    let available_milestones: Vec<RepoMilestone> = repo_data
        .milestones
        .nodes
        .iter()
        .map(|m| m.clone().into())
        .collect();

    let mut issue_details = IssueDetails::builder()
        .owner(owner.clone())
        .repo(repo.clone())
        .number(number)
        .title(issue_node.title)
        .body(issue_node.body.unwrap_or_default())
        .labels(labels)
        .comments(comments)
        .url(issue_node.url)
        .available_labels(available_labels)
        .available_milestones(available_milestones)
        .build();

    // Extract keywords and language for parallel calls
    let keywords = issues::extract_keywords(&issue_details.title);
    let language = repo_data
        .primary_language
        .as_ref()
        .map_or("unknown", |l| l.name.as_str())
        .to_string();

    // Run search and tree fetch in parallel - both depend on GraphQL results but not each other
    let (search_result, tree_result) = tokio::join!(
        issues::search_related_issues(&client, &owner, &repo, &issue_details.title, number),
        issues::fetch_repo_tree(&client, &owner, &repo, &language, &keywords)
    );

    // Handle search results
    match search_result {
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

    // Handle tree results
    match tree_result {
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
pub async fn analyze(
    issue_details: &IssueDetails,
    ai_config: &aptu_core::AiConfig,
) -> Result<AiResponse> {
    // Create CLI token provider
    let provider = crate::provider::CliTokenProvider;

    // Call facade for analysis
    let ai_response = aptu_core::analyze_issue(&provider, issue_details, ai_config).await?;

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

/// Apply AI-suggested labels and milestone to an issue.
///
/// Labels are applied additively: existing labels are preserved and AI-suggested labels are merged in.
/// Priority labels (p1/p2/p3) defer to existing human judgment.
/// Milestones are only set if the issue doesn't already have one.
/// Validates suggestions against available labels and milestones before applying.
/// Returns what was applied and any warnings.
///
/// # Arguments
///
/// * `issue_details` - Issue details including available labels and milestones
/// * `triage` - AI triage response with suggestions
#[instrument(skip_all, fields(owner = %issue_details.owner, repo = %issue_details.repo, number = issue_details.number))]
pub async fn apply(
    issue_details: &IssueDetails,
    triage: &TriageResponse,
) -> Result<issues::ApplyResult> {
    debug!("Applying labels and milestone to issue");

    // Check authentication
    if !auth::is_authenticated() {
        return Err(AptuError::NotAuthenticated.into());
    }

    // Create authenticated client
    let client = auth::create_client()?;

    // Call the update function with validation
    let result = issues::update_issue_labels_and_milestone(
        &client,
        &issue_details.owner,
        &issue_details.repo,
        issue_details.number,
        &issue_details.labels,
        &triage.suggested_labels,
        issue_details.milestone.as_deref(),
        triage.suggested_milestone.as_deref(),
        &issue_details.available_labels,
        &issue_details.available_milestones,
    )
    .await?;

    info!(
        labels = ?result.applied_labels,
        milestone = ?result.applied_milestone,
        warnings = ?result.warnings,
        "Labels and milestone applied"
    );

    Ok(result)
}
