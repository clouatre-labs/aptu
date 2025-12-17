//! Triage an issue with AI assistance command.
//!
//! Fetches a GitHub issue, analyzes it with AI, and optionally posts
//! a comment to GitHub. Split into `analyze()` and `post()` for proper
//! confirmation flow (render before asking).

use anyhow::{Context, Result};
use aptu_core::ai::openrouter::analyze_issue;
use aptu_core::ai::types::TriageResponse;
use aptu_core::config::load_config;
use aptu_core::github::{auth, issues};
use tracing::{debug, info, instrument};

use crate::output::render_triage_markdown;

/// Intermediate result from analysis (before posting decision).
pub struct AnalyzeResult {
    /// Issue title.
    pub issue_title: String,
    /// Issue number.
    pub issue_number: u64,
    /// AI triage analysis.
    pub triage: TriageResponse,
    /// Repository owner.
    pub owner: String,
    /// Repository name.
    pub repo: String,
}

/// Analyze an issue with AI assistance.
///
/// Fetches issue details and runs AI analysis. Does not post anything.
///
/// # Arguments
///
/// * `reference` - Issue reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
#[instrument(skip_all, fields(reference = %reference))]
pub async fn analyze(reference: &str, repo_context: Option<&str>) -> Result<AnalyzeResult> {
    // Load configuration
    let config = load_config().context("Failed to load configuration")?;

    // Check authentication
    if !auth::is_authenticated() {
        anyhow::bail!("Authentication required - run `aptu auth login` first");
    }

    // Parse the issue reference
    let (owner, repo, number) = issues::parse_issue_reference(reference, repo_context)?;

    // Create authenticated client
    let client = auth::create_client().context("Failed to create GitHub client")?;

    // Fetch issue details
    let issue_details = issues::fetch_issue_with_comments(&client, &owner, &repo, number).await?;

    // Call AI for analysis
    let triage = analyze_issue(&config.ai, &issue_details).await?;

    Ok(AnalyzeResult {
        issue_title: issue_details.title,
        issue_number: number,
        triage,
        owner,
        repo,
    })
}

/// Post a triage comment to GitHub.
#[instrument(skip_all, fields(issue_number = analyze_result.issue_number))]
pub async fn post(analyze_result: &AnalyzeResult) -> Result<String> {
    // Create authenticated client
    let client = auth::create_client().context("Failed to create GitHub client")?;

    // Post the comment
    let comment_body = render_triage_markdown(&analyze_result.triage);
    let comment_url = issues::post_comment(
        &client,
        &analyze_result.owner,
        &analyze_result.repo,
        analyze_result.issue_number,
        &comment_body,
    )
    .await?;

    info!(comment_url = %comment_url, "Triage comment posted");
    debug!("Triage complete for issue #{}", analyze_result.issue_number);

    Ok(comment_url)
}
