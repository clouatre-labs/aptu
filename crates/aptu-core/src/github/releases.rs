// SPDX-License-Identifier: Apache-2.0

//! Release-related GitHub operations.
//!
//! Provides functions for fetching PRs between git tags and parsing tag references.

use anyhow::{Context, Result};
use octocrab::params::repos::Reference;
use tracing::instrument;

use crate::ai::types::PrSummary;

/// Fetch merged PRs between two git references.
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `from_ref` - Starting reference (tag or commit)
/// * `to_ref` - Ending reference (tag or commit)
///
/// # Returns
///
/// Vector of `PrSummary` for merged PRs between the references.
#[instrument(skip(client))]
pub async fn fetch_prs_between_refs(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    from_ref: &str,
    to_ref: &str,
) -> Result<Vec<PrSummary>> {
    // Get the commit SHAs for the references
    let from_sha = resolve_ref_to_sha(client, owner, repo, from_ref).await?;
    let to_sha = resolve_ref_to_sha(client, owner, repo, to_ref).await?;

    // Fetch all merged PRs
    let mut prs = Vec::new();
    let mut page = 1u32;

    loop {
        let pulls = client
            .pulls(owner, repo)
            .list()
            .state(octocrab::params::State::Closed)
            .per_page(100)
            .page(page)
            .send()
            .await
            .context("Failed to fetch PRs from GitHub")?;

        if pulls.items.is_empty() {
            break;
        }

        for pr in &pulls.items {
            // Only include merged PRs
            if pr.merged_at.is_none() {
                continue;
            }

            // Check if PR is between the two refs
            if let Some(merge_commit) = &pr.merge_commit_sha
                && is_commit_between(client, owner, repo, &from_sha, &to_sha, merge_commit).await?
            {
                prs.push(PrSummary {
                    number: pr.number,
                    title: pr.title.clone().unwrap_or_default(),
                    body: pr.body.clone().unwrap_or_default(),
                    author: pr
                        .user
                        .as_ref()
                        .map_or_else(|| "unknown".to_string(), |u| u.login.clone()),
                    merged_at: pr.merged_at.map(|dt| dt.to_rfc3339()),
                });
            }
        }

        // Check if there are more pages
        if pulls.items.len() < 100 {
            break;
        }

        page += 1;
    }

    Ok(prs)
}

/// Resolve a git reference (tag or commit) to its SHA.
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `ref_name` - Reference name (tag or commit SHA)
///
/// # Returns
///
/// The commit SHA for the reference.
#[instrument(skip(client))]
async fn resolve_ref_to_sha(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    ref_name: &str,
) -> Result<String> {
    // Try to get the reference as a tag first
    let tag_ref = Reference::Tag(ref_name.to_string());
    match client.repos(owner, repo).get_ref(&tag_ref).await {
        Ok(git_ref) => {
            // Extract SHA from the ref object
            if let octocrab::models::repos::Object::Commit { sha, .. } = git_ref.object {
                Ok(sha)
            } else {
                Err(anyhow::anyhow!("Expected commit object for tag {ref_name}"))
            }
        }
        Err(_) => {
            // If tag not found, assume it's a commit SHA and return as-is
            Ok(ref_name.to_string())
        }
    }
}

/// Check if a commit is between two references using GitHub Compare API.
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `from_sha` - Starting commit SHA
/// * `to_sha` - Ending commit SHA
/// * `commit_sha` - Commit SHA to check
///
/// # Returns
///
/// True if the commit is between the two references.
#[instrument(skip(client))]
async fn is_commit_between(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    from_sha: &str,
    to_sha: &str,
    commit_sha: &str,
) -> Result<bool> {
    // Use GitHub Compare API to get commits between two refs
    // GET /repos/{owner}/{repo}/compare/{base}...{head}
    let route = format!("repos/{owner}/{repo}/compare/{from_sha}...{to_sha}");

    #[derive(serde::Deserialize)]
    struct CompareResponse {
        commits: Vec<CommitInfo>,
    }

    #[derive(serde::Deserialize)]
    struct CommitInfo {
        sha: String,
    }

    let comparison: CompareResponse = client
        .get(&route, None::<&()>)
        .await
        .context("Failed to compare commits")?;

    // Check if the commit is in the list of commits between the refs
    Ok(comparison.commits.iter().any(|c| c.sha == commit_sha))
}

/// Get the latest tag in a repository.
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
///
/// # Returns
///
/// The latest tag name and its SHA.
#[instrument(skip(client))]
pub async fn get_latest_tag(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
) -> Result<(String, String)> {
    let releases = client
        .repos(owner, repo)
        .releases()
        .list()
        .per_page(1)
        .send()
        .await
        .context("Failed to fetch releases from GitHub")?;

    if releases.items.is_empty() {
        anyhow::bail!("No releases found in repository");
    }

    let latest = &releases.items[0];
    let tag_name = latest.tag_name.clone();

    // Get the commit SHA for the tag
    let tag_ref = Reference::Tag(tag_name.clone());
    let git_ref = client
        .repos(owner, repo)
        .get_ref(&tag_ref)
        .await
        .context(format!("Failed to get tag reference: {tag_name}"))?;

    // Extract SHA from the ref object
    let octocrab::models::repos::Object::Commit { sha, .. } = git_ref.object else {
        anyhow::bail!("Expected commit object for tag {tag_name}")
    };

    Ok((tag_name, sha))
}

/// Parse a tag reference to extract the version.
///
/// Handles common tag formats like v1.0.0, 1.0.0, release-1.0.0, etc.
///
/// # Arguments
///
/// * `tag` - The tag name to parse
///
/// # Returns
///
/// The version string extracted from the tag.
#[must_use]
pub fn parse_tag_reference(tag: &str) -> String {
    // Remove common prefixes (check longer prefixes first)
    let version = tag
        .strip_prefix("release-")
        .or_else(|| tag.strip_prefix("v-"))
        .or_else(|| tag.strip_prefix('v'))
        .unwrap_or(tag);

    version.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tag_reference_v_prefix() {
        assert_eq!(parse_tag_reference("v1.0.0"), "1.0.0");
    }

    #[test]
    fn test_parse_tag_reference_release_prefix() {
        assert_eq!(parse_tag_reference("release-1.0.0"), "1.0.0");
    }

    #[test]
    fn test_parse_tag_reference_v_dash_prefix() {
        assert_eq!(parse_tag_reference("v-1.0.0"), "1.0.0");
    }

    #[test]
    fn test_parse_tag_reference_no_prefix() {
        assert_eq!(parse_tag_reference("1.0.0"), "1.0.0");
    }
}
