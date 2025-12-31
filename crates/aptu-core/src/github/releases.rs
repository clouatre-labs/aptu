// SPDX-License-Identifier: Apache-2.0

//! Release-related GitHub operations.
//!
//! Provides functions for fetching PRs between git tags and parsing tag references.

use anyhow::{Context, Result};
use std::collections::HashSet;
use tracing::instrument;

use crate::ai::types::PrSummary;

#[derive(serde::Deserialize)]
struct RefResponse {
    object: RefObject,
}

#[derive(serde::Deserialize)]
struct RefObject {
    sha: String,
}

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

    // Fetch all commits between refs upfront using Compare API with pagination
    let commit_shas = fetch_commits_between_refs(client, owner, repo, &from_sha, &to_sha).await?;

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

            // Check if PR is between the two refs using local HashSet lookup
            if let Some(merge_commit) = &pr.merge_commit_sha
                && commit_shas.contains(merge_commit)
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
    // Try to resolve as a tag using GraphQL first
    match super::graphql::resolve_tag_to_commit_sha(client, owner, repo, ref_name).await? {
        Some(sha) => Ok(sha),
        None => {
            // If GraphQL returns None, try REST API as fallback
            // This handles cases where tags are recreated and GraphQL cache is stale
            match resolve_tag_via_rest(client, owner, repo, ref_name).await {
                Ok(sha) => Ok(sha),
                Err(_) => {
                    // If both GraphQL and REST API fail, assume it's a commit SHA
                    Ok(ref_name.to_string())
                }
            }
        }
    }
}

/// Resolve a tag to its commit SHA using the REST API.
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `tag_name` - Tag name to resolve
///
/// # Returns
///
/// The commit SHA for the tag, or an error if the tag doesn't exist.
#[instrument(skip(client))]
async fn resolve_tag_via_rest(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    tag_name: &str,
) -> Result<String> {
    let route = format!("/repos/{owner}/{repo}/git/refs/tags/{tag_name}");

    let response: RefResponse = client
        .get::<RefResponse, &str, ()>(&route, None::<&()>)
        .await
        .context(format!("Failed to resolve tag {tag_name} via REST API"))?;

    Ok(response.object.sha)
}

/// Fetch all commits between two references using GitHub Compare API with pagination.
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `from_sha` - Starting commit SHA
/// * `to_sha` - Ending commit SHA
///
/// # Returns
///
/// `HashSet` of commit SHAs between the two references.
#[instrument(skip(client))]
async fn fetch_commits_between_refs(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    from_sha: &str,
    to_sha: &str,
) -> Result<HashSet<String>> {
    #[derive(serde::Deserialize)]
    struct CompareResponse {
        commits: Vec<CommitInfo>,
    }

    #[derive(serde::Deserialize)]
    struct CommitInfo {
        sha: String,
    }

    let mut commit_shas = HashSet::new();
    let mut page = 1u32;

    loop {
        // Use GitHub Compare API to get commits between two refs with pagination
        // GET /repos/{owner}/{repo}/compare/{base}...{head}?per_page=100&page={page}
        let route =
            format!("/repos/{owner}/{repo}/compare/{from_sha}...{to_sha}?per_page=100&page={page}");

        let comparison: CompareResponse = client
            .get(&route, None::<&()>)
            .await
            .context("Failed to compare commits")?;

        let count = comparison.commits.len();
        commit_shas.extend(comparison.commits.into_iter().map(|c| c.sha));

        // Check if there are more pages
        if count < 100 {
            break;
        }

        page += 1;
    }

    Ok(commit_shas)
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
/// The latest tag name and its SHA, or None if no releases exist.
#[instrument(skip(client))]
pub async fn get_latest_tag(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
) -> Result<Option<(String, String)>> {
    let releases = client
        .repos(owner, repo)
        .releases()
        .list()
        .per_page(1)
        .send()
        .await
        .context("Failed to fetch releases from GitHub")?;

    if releases.items.is_empty() {
        return Ok(None);
    }

    let latest = &releases.items[0];
    let tag_name = latest.tag_name.clone();

    // Get the commit SHA for the tag using GraphQL
    match super::graphql::resolve_tag_to_commit_sha(client, owner, repo, &tag_name).await? {
        Some(sha) => Ok(Some((tag_name, sha))),
        None => anyhow::bail!("Failed to resolve tag {tag_name} to commit SHA"),
    }
}

/// Get the root (oldest) commit in a repository.
///
/// Uses the GitHub API compare endpoint with the empty tree SHA to fetch all commits
/// in reverse chronological order, then returns the oldest (last) commit.
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
///
/// # Returns
///
/// The SHA of the root commit.
#[instrument(skip(client))]
pub async fn get_root_commit(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
) -> Result<String> {
    // Empty tree SHA - represents the initial state before any commits
    const EMPTY_TREE_SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

    // Use compare endpoint to get all commits from empty tree to HEAD
    // This returns commits in chronological order (oldest first)
    // GET /repos/{owner}/{repo}/compare/{base}...{head}
    let route = format!("/repos/{owner}/{repo}/compare/{EMPTY_TREE_SHA}...HEAD");

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
        .context("Failed to fetch commits from GitHub")?;

    if comparison.commits.is_empty() {
        anyhow::bail!("Repository has no commits");
    }

    // The first commit in the list is the oldest (root) commit
    let root_commit = &comparison.commits[0];
    Ok(root_commit.sha.clone())
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

/// Post release notes to GitHub.
///
/// Creates or updates a release on GitHub with the provided body.
/// If the release already exists, it will be updated. Otherwise, a new release is created.
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `tag` - The tag name for the release
/// * `body` - The release notes body
///
/// # Returns
///
/// The URL of the created or updated release.
#[instrument(skip(client))]
pub async fn post_release_notes(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    tag: &str,
    body: &str,
) -> Result<String> {
    let repo_handle = client.repos(owner, repo);
    let releases = repo_handle.releases();

    // Try to get existing release by tag
    if let Ok(existing_release) = releases.get_by_tag(tag).await {
        // Update existing release
        let updated = releases
            .update(existing_release.id.0)
            .body(body)
            .send()
            .await
            .context(format!("Failed to update release for tag {tag}"))?;

        Ok(updated.html_url.to_string())
    } else {
        // Create new release
        let created = releases
            .create(tag)
            .body(body)
            .send()
            .await
            .context(format!("Failed to create release for tag {tag}"))?;

        Ok(created.html_url.to_string())
    }
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
