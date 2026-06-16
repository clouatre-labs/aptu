// SPDX-License-Identifier: Apache-2.0

//! Revert facade functions for undoing aptu-authored changes.

#[cfg(not(target_arch = "wasm32"))]
use octocrab::Octocrab;
use tracing::{debug, instrument};

use crate::error::AptuError;
#[cfg(not(target_arch = "wasm32"))]
use crate::github::pulls::fetch_pr_details;

/// Result from reverting issue comments and labels.
#[derive(Debug, Clone)]
pub struct RevertOutcome {
    /// Whether this was a dry-run.
    pub dry_run: bool,
    /// Labels that were removed.
    pub labels_removed: Vec<String>,
    /// IDs of comments that were removed.
    pub comment_ids: Vec<u64>,
}

/// Reverts all comments and labels posted by the authenticated aptu user on an issue.
///
/// Fetches the issue with comments, identifies comments authored by the authenticated user,
/// and deletes them along with any labels (if not in dry-run mode).
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `number` - Issue number
/// * `dry_run` - If true, preview only without making deletions
///
/// # Returns
///
/// A `RevertOutcome` describing what was/would be removed.
///
/// # Errors
///
/// Returns an error if GitHub API calls fail or authentication fails.
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number, dry_run))]
pub async fn revert_issue(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
    dry_run: bool,
) -> crate::Result<RevertOutcome> {
    use crate::github::issues::{
        delete_issue_comment, fetch_issue_with_comments, remove_issue_label,
    };

    debug!("Reverting issue comments and labels");

    // Get authenticated user login
    let authenticated_user = client
        .current()
        .user()
        .await
        .map_err(|e| AptuError::GitHub {
            message: format!("Failed to get authenticated user: {e}"),
        })?;
    let auth_login = authenticated_user.login.clone();
    debug!(auth_login = %auth_login, "Authenticated as user");

    // Fetch issue with all comments
    let issue_details = fetch_issue_with_comments(client, owner, repo, number)
        .await
        .map_err(|e| AptuError::GitHub {
            message: format!("Failed to fetch issue: {e}"),
        })?;

    // Identify comments authored by the authenticated user
    let mut comment_ids_to_delete = Vec::new();
    for comment in &issue_details.comments {
        if comment.author == auth_login {
            comment_ids_to_delete.push(comment.id);
            debug!(comment_id = comment.id, "Found aptu-authored comment");
        }
    }

    // Collect all labels from the issue
    let labels_to_remove: Vec<String> = issue_details.labels.clone();

    if dry_run {
        debug!(
            comment_count = comment_ids_to_delete.len(),
            label_count = labels_to_remove.len(),
            "Dry-run mode: no deletions will be performed"
        );
        return Ok(RevertOutcome {
            dry_run: true,
            labels_removed: labels_to_remove,
            comment_ids: comment_ids_to_delete,
        });
    }

    // Delete comments
    for comment_id in &comment_ids_to_delete {
        if let Err(e) = delete_issue_comment(client, owner, repo, *comment_id).await {
            return Err(AptuError::GitHub {
                message: format!("Failed to delete comment #{comment_id}: {e}"),
            });
        }
    }
    debug!(count = comment_ids_to_delete.len(), "Comments deleted");

    // Remove labels
    for label in &labels_to_remove {
        if let Err(e) = remove_issue_label(client, owner, repo, number, label).await {
            return Err(AptuError::GitHub {
                message: format!("Failed to remove label '{label}': {e}"),
            });
        }
    }
    debug!(count = labels_to_remove.len(), "Labels removed");

    Ok(RevertOutcome {
        dry_run: false,
        labels_removed: labels_to_remove,
        comment_ids: comment_ids_to_delete,
    })
}

/// Reverts all comments and labels posted by the authenticated aptu user on a PR.
///
/// Fetches the PR with review comments, identifies comments authored by the authenticated user,
/// and deletes them along with any labels (if not in dry-run mode).
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `number` - PR number
/// * `dry_run` - If true, preview only without making deletions
///
/// # Returns
///
/// A `RevertOutcome` describing what was/would be removed.
///
/// # Errors
///
/// Returns an error if GitHub API calls fail or authentication fails.
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number, dry_run))]
pub async fn revert_pr(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
    dry_run: bool,
) -> crate::Result<RevertOutcome> {
    use crate::github::issues::remove_issue_label;
    use crate::github::pulls::delete_pr_review_comment;

    debug!("Reverting PR comments and labels");

    // Get authenticated user login
    let authenticated_user = client
        .current()
        .user()
        .await
        .map_err(|e| AptuError::GitHub {
            message: format!("Failed to get authenticated user: {e}"),
        })?;
    let auth_login = authenticated_user.login.clone();
    debug!(auth_login = %auth_login, "Authenticated as user");

    // Fetch PR details with review comments
    let pr_details = fetch_pr_details(
        client,
        owner,
        repo,
        number,
        &crate::config::ReviewConfig::default(),
    )
    .await
    .map_err(|e| AptuError::GitHub {
        message: format!("Failed to fetch PR: {e}"),
    })?;

    // Identify review comments authored by the authenticated user
    let mut comment_ids_to_delete = Vec::new();
    for comment in &pr_details.review_comments {
        if comment.author == auth_login {
            comment_ids_to_delete.push(comment.id);
            debug!(
                comment_id = comment.id,
                "Found aptu-authored review comment"
            );
        }
    }

    // Collect all labels from the PR
    let labels_to_remove: Vec<String> = pr_details.labels.clone();

    if dry_run {
        debug!(
            comment_count = comment_ids_to_delete.len(),
            label_count = labels_to_remove.len(),
            "Dry-run mode: no deletions will be performed"
        );
        return Ok(RevertOutcome {
            dry_run: true,
            labels_removed: labels_to_remove,
            comment_ids: comment_ids_to_delete,
        });
    }

    // Delete review comments
    for comment_id in &comment_ids_to_delete {
        if let Err(e) = delete_pr_review_comment(client, owner, repo, *comment_id).await {
            return Err(AptuError::GitHub {
                message: format!("Failed to delete PR review comment #{comment_id}: {e}"),
            });
        }
    }
    debug!(
        count = comment_ids_to_delete.len(),
        "PR review comments deleted"
    );

    // Remove labels
    for label in &labels_to_remove {
        if let Err(e) = remove_issue_label(client, owner, repo, number, label).await {
            return Err(AptuError::GitHub {
                message: format!("Failed to remove label '{label}': {e}"),
            });
        }
    }
    debug!(count = labels_to_remove.len(), "Labels removed from PR");

    Ok(RevertOutcome {
        dry_run: false,
        labels_removed: labels_to_remove,
        comment_ids: comment_ids_to_delete,
    })
}
