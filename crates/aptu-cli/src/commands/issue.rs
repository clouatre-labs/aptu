//! List open issues command.
//!
//! Fetches "good first issue" issues from curated repositories using
//! a single GraphQL query for optimal performance.

use anyhow::{Context, Result};
use aptu_core::github::{auth, graphql};
use aptu_core::repos;
use tracing::{debug, info, instrument};

use super::types::IssuesResult;

/// List open issues suitable for contribution.
///
/// Fetches issues with "good first issue" label from all curated repositories
/// (or a specific one if `--repo` is provided).
#[instrument(skip_all, fields(repo_filter = ?repo))]
pub async fn run(repo: Option<String>) -> Result<IssuesResult> {
    // Check authentication
    if !auth::is_authenticated() {
        anyhow::bail!("Authentication required - run `aptu auth login` first");
    }

    // Get curated repos, optionally filtered
    let all_repos = repos::list();
    let repos_to_query: Vec<_> = match &repo {
        Some(filter) => {
            let filter_lower = filter.to_lowercase();
            all_repos
                .iter()
                .filter(|r| {
                    r.full_name().to_lowercase().contains(&filter_lower)
                        || r.name.to_lowercase().contains(&filter_lower)
                })
                .cloned()
                .collect()
        }
        None => all_repos.to_vec(),
    };

    if repos_to_query.is_empty() {
        return Ok(IssuesResult {
            issues_by_repo: vec![],
            total_count: 0,
            repo_filter: repo,
            no_repos_matched: true,
        });
    }

    // Create authenticated client
    let client = auth::create_client().context("Failed to create GitHub client")?;

    // Fetch issues via GraphQL
    let results = graphql::fetch_issues(&client, &repos_to_query).await?;

    // Count total issues
    let total_count: usize = results.iter().map(|(_, issues)| issues.len()).sum();

    info!(
        total_issues = total_count,
        repos = results.len(),
        "Found issues"
    );
    debug!("Issues listing complete");

    Ok(IssuesResult {
        issues_by_repo: results,
        total_count,
        repo_filter: repo,
        no_repos_matched: false,
    })
}
