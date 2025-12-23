// SPDX-License-Identifier: Apache-2.0

//! Create a GitHub issue with AI assistance command.
//!
//! Supports interactive mode for title/body input, reading from files,
//! and AI formatting. Returns created issue details.

use std::fs;
use std::io::IsTerminal;

use anyhow::{Context, Result};
use dialoguer::Input;
use tracing::{debug, info, instrument};

use super::types::CreateResult;
use aptu_core::github::auth;
use aptu_core::github::issues as gh_issues;

/// Create a GitHub issue with AI assistance.
///
/// Handles interactive mode, file reading, AI formatting, and GitHub posting.
/// Returns `CreateResult` with issue details and suggested labels.
///
/// # Arguments
///
/// * `repo` - Repository in owner/repo format
/// * `title` - Optional issue title (interactive prompt if None)
/// * `body` - Optional issue body (interactive prompt if None)
/// * `from` - Optional file path to read content from
/// * `dry_run` - If true, preview without posting to GitHub
#[instrument(skip_all, fields(repo = %repo, dry_run = %dry_run))]
pub async fn run(
    repo: String,
    title: Option<String>,
    body: Option<String>,
    from: Option<String>,
    dry_run: bool,
) -> Result<CreateResult> {
    // Parse owner/repo
    let (owner, repo_name) = gh_issues::parse_owner_repo(&repo)?;
    debug!(owner = %owner, repo = %repo_name, "Parsed repository");

    // Get title and body
    let (final_title, final_body) = if let Some(file_path) = &from {
        // Read from file
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {file_path}"))?;
        debug!(file_path = %file_path, "Read content from file");

        // Use file content as body, let AI handle title extraction
        let t = title.clone().unwrap_or_else(|| "Untitled".to_string());
        let b = body.clone().unwrap_or(content);
        (t, b)
    } else if let (Some(t), Some(b)) = (&title, &body) {
        // Both title and body provided via flags
        (t.clone(), b.clone())
    } else if let Some(t) = &title {
        // Only title provided, prompt for body
        let b = prompt_body()?;
        (t.clone(), b)
    } else if let Some(b) = &body {
        // Only body provided, prompt for title
        let t = prompt_title()?;
        (t, b.clone())
    } else {
        // Neither provided, prompt for both
        let t = prompt_title()?;
        let b = prompt_body()?;
        (t, b)
    };

    debug!(title = %final_title, body_len = final_body.len(), "Collected issue content");

    // Format title and body with AI
    let ai_response = aptu_core::ai::create_issue(&final_title, &final_body, &repo)
        .await
        .context("Failed to format issue with AI")?;

    debug!(
        formatted_title = %ai_response.formatted_title,
        labels_count = ai_response.suggested_labels.len(),
        "AI formatting complete"
    );

    // If dry run, return result without posting
    if dry_run {
        info!("Dry run mode: skipping GitHub API call");
        return Ok(CreateResult {
            issue_url: format!("https://github.com/{owner}/{repo_name}/issues/[preview]"),
            issue_number: 0,
            title: ai_response.formatted_title,
            body: ai_response.formatted_body,
            suggested_labels: ai_response.suggested_labels,
            dry_run: true,
        });
    }

    // Check authentication
    if !auth::is_authenticated() {
        anyhow::bail!("Not authenticated. Run 'aptu auth login' first.");
    }

    // Create GitHub client
    let client = auth::create_client().context("Failed to create GitHub client")?;

    // Post issue to GitHub
    let (issue_url, issue_number) = gh_issues::create_issue(
        &client,
        &owner,
        &repo_name,
        &ai_response.formatted_title,
        &ai_response.formatted_body,
    )
    .await
    .context("Failed to create GitHub issue")?;

    info!(issue_number = issue_number, "Issue created successfully");

    Ok(CreateResult {
        issue_url,
        issue_number,
        title: ai_response.formatted_title,
        body: ai_response.formatted_body,
        suggested_labels: ai_response.suggested_labels,
        dry_run: false,
    })
}

/// Prompt user for issue title interactively.
///
/// Uses `dialoguer::Input` with validation. Requires TTY.
///
/// # Errors
///
/// Returns error if not in TTY or user cancels input.
fn prompt_title() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Interactive mode requires a terminal. Use --title flag instead.");
    }

    let title = Input::<String>::new()
        .with_prompt("Issue title")
        .validate_with(|input: &String| {
            if input.is_empty() {
                Err("Title cannot be empty")
            } else if input.len() > 256 {
                Err("Title must be 256 characters or less")
            } else {
                Ok(())
            }
        })
        .interact()
        .context("Failed to read title from input")?;

    Ok(title)
}

/// Prompt user for issue body interactively.
///
/// Uses `dialoguer::Input` with basic validation. Requires TTY.
///
/// # Errors
///
/// Returns error if not in TTY or user cancels input.
fn prompt_body() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Interactive mode requires a terminal. Use --body flag instead.");
    }

    let body = Input::<String>::new()
        .with_prompt("Issue description (press Enter twice to finish, or use --body for multiline)")
        .allow_empty(false)
        .interact()
        .context("Failed to read body from input")?;

    Ok(body)
}
