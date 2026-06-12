// SPDX-License-Identifier: Apache-2.0

//! PR creation facade functions.

use tracing::instrument;

use crate::auth::TokenProvider;
use crate::error::AptuError;
use crate::github::auth::create_client_from_provider;

/// Creates a pull request on GitHub.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `title` - PR title
/// * `base_branch` - Base branch (the branch to merge into)
/// * `head_branch` - Head branch (the branch with changes)
/// * `body` - Optional PR body text
///
/// # Returns
///
/// `PrCreateResult` with PR metadata.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - GitHub API call fails
/// - User lacks write access to the repository
#[instrument(skip(provider), fields(owner = %owner, repo = %repo, head = %head_branch, base = %base_branch))]
#[allow(clippy::too_many_arguments)]
pub async fn create_pr(
    provider: &dyn TokenProvider,
    owner: &str,
    repo: &str,
    title: &str,
    base_branch: &str,
    head_branch: &str,
    body: Option<&str>,
    draft: bool,
) -> crate::Result<crate::github::pulls::PrCreateResult> {
    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Create the pull request
    crate::github::pulls::create_pull_request(
        &client,
        owner,
        repo,
        title,
        head_branch,
        base_branch,
        body,
        draft,
    )
    .await
    .map_err(|e| AptuError::GitHub {
        message: e.to_string(),
    })
}
