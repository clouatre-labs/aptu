// SPDX-License-Identifier: Apache-2.0

//! Review context assembly: CWD inference, AST/call-graph building, and enrichment.

use std::path::PathBuf;

use super::ReviewContext;
use super::budget::{apply_budget_drops, estimate_pr_size, should_enable_call_graph};
use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

/// Builds a `ReviewContext` by centralizing all enrichment decisions.
///
/// This function owns:
/// - CWD inference logic (moved from `facade.rs`)
/// - AST context building (moved from `facade.rs`)
/// - Call graph auto-enable logic (moved from `review_pr()`)
/// - Dependency enrichment (moved from `review_pr()`)
/// - Budget drop order enforcement
///
/// # Arguments
///
/// * `pr` - Pull request details
/// * `repo_path` - Optional explicit repository path (overrides CWD inference)
/// * `review_config` - Review configuration with budget thresholds
///
/// # Returns
///
/// A `ReviewContext` with all enrichment fields populated according to budget constraints.
pub async fn build_review_context(
    mut pr: PrDetails,
    repo_path: Option<String>,
    review_config: &ReviewConfig,
) -> crate::Result<ReviewContext> {
    // Step 1: Resolve repo_path (explicit or inferred from CWD)
    #[cfg(not(target_arch = "wasm32"))]
    let (inferred_repo_path, cwd_inferred) = resolve_repo_path(&pr, repo_path);
    #[cfg(target_arch = "wasm32")]
    let (inferred_repo_path, cwd_inferred) = (repo_path.map(std::path::PathBuf::from), false);
    let repo_path_ref = inferred_repo_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());

    // Step 2: Build AST context if repo_path resolved
    // When `ast-context` feature is enabled, build_ctx_ast returns AstContextOutput
    // (which carries the text string); when disabled, it returns a plain String.
    #[cfg(feature = "ast-context")]
    let ast_context = build_ctx_ast(repo_path_ref.as_deref(), &pr.files)
        .await
        .text;
    #[cfg(not(feature = "ast-context"))]
    let ast_context = build_ctx_ast(repo_path_ref.as_deref(), &pr.files).await;

    // Step 3: Enrich with dependency release notes
    pr.dep_enrichments = enrich_deps(&pr.files, review_config).await;

    // Step 4: Estimate total chars and decide call_graph budget
    // (call_graph not yet built, pass empty)
    let estimated_size = estimate_pr_size(&pr, &ast_context, "");
    let max_prompt_chars = review_config.max_prompt_chars;
    let budget_remaining = max_prompt_chars.saturating_sub(estimated_size);

    // Step 5: Build call_graph if decided
    let should_enable_cg = should_enable_call_graph(budget_remaining, review_config);
    let mut call_graph = if should_enable_cg {
        build_ctx_call_graph(repo_path_ref.as_deref(), &pr.files).await
    } else {
        String::new()
    };

    // Re-estimate with actual call_graph for accurate routing
    let final_estimated_size = estimate_pr_size(&pr, &ast_context, &call_graph);

    // Step 6: Apply budget drop order
    let had_patches_pre_drop = pr
        .files
        .iter()
        .any(|f| f.patch.as_deref().is_some_and(|p| !p.is_empty()));
    let mut ast_context = ast_context;
    let mut budget_drops = Vec::new();
    apply_budget_drops(
        &mut pr,
        &mut ast_context,
        &mut call_graph,
        max_prompt_chars,
        &mut budget_drops,
        repo_path_ref.as_deref(),
    );

    // Collect tracking metrics
    let files_total = pr.files.len();
    let files_with_patch = pr
        .files
        .iter()
        .filter(|f| f.patch.as_deref().is_some_and(|p| !p.is_empty()))
        .count();
    let dep_enrichments_count = pr.dep_enrichments.len();
    let dep_enrichments_chars = pr
        .dep_enrichments
        .iter()
        .map(|d| serde_json::to_string(d).unwrap_or_default().len())
        .sum();

    if had_patches_pre_drop && files_with_patch == 0 {
        tracing::warn!(
            pr_owner = %pr.owner,
            pr_repo = %pr.repo,
            pr_number = pr.number,
            files_total,
            files_with_patch,
            had_patches_pre_drop,
            "Review context has no surviving patches; refusing diff-less review"
        );
        return Err(crate::AptuError::EmptyReviewContext { files_total });
    }

    Ok(ReviewContext {
        pr,
        ast_context,
        call_graph,
        inferred_repo_path,
        cwd_inferred,
        max_chars_per_file: review_config.max_chars_per_file,
        max_diff_chars: review_config.max_diff_chars,
        max_patch_chars_per_file: review_config.max_patch_chars_per_file,
        files_truncated: 0,
        truncated_chars_dropped: 0,
        files_total,
        files_with_patch,
        dep_enrichments_count,
        dep_enrichments_chars,
        budget_drops,
        prompt_chars_final: 0,
        estimated_size: final_estimated_size,
    })
}

/// Resolves the repository path from explicit argument or CWD inference.
///
/// Returns a tuple of `(inferred_repo_path, cwd_inferred)`.
#[cfg(not(target_arch = "wasm32"))]
fn resolve_repo_path(
    pr: &PrDetails,
    explicit_repo_path: Option<String>,
) -> (Option<PathBuf>, bool) {
    if explicit_repo_path.is_some() {
        (explicit_repo_path.map(PathBuf::from), false)
    } else if let Some(inferred_path) = infer_repo_path_from_cwd(&pr.owner, &pr.repo) {
        (Some(PathBuf::from(&inferred_path)), true)
    } else {
        (None, false)
    }
}

/// Enriches PR with dependency release notes if manifest files are detected.
pub(crate) async fn enrich_deps(
    files: &[crate::ai::types::PrFile],
    config: &ReviewConfig,
) -> Vec<crate::ai::types::DepReleaseNote> {
    crate::ai::dep_enrichment::enrich_dep_releases(
        files,
        config.max_dep_packages,
        config.max_dep_release_chars,
    )
    .await
}

/// Builds AST context for changed files.
///
/// Returns an [`AstContextOutput`] containing the text string built from analysis.
#[allow(clippy::unused_async)]
#[cfg(feature = "ast-context")]
pub(crate) async fn build_ctx_ast(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
) -> crate::ast_context::AstContextOutput {
    let Some(path) = repo_path else {
        return crate::ast_context::AstContextOutput::new(String::new());
    };
    crate::ast_context::build_ast_context(path, files).await
}

/// Builds AST context for changed files (stub when `ast-context` feature is off).
#[allow(clippy::unused_async)]
#[cfg(not(feature = "ast-context"))]
pub(crate) async fn build_ctx_ast(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
) -> String {
    let _ = (repo_path, files);
    String::new()
}

/// Builds call-graph context for changed files.
#[allow(clippy::unused_async)]
pub(crate) async fn build_ctx_call_graph(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
) -> String {
    let Some(path) = repo_path else {
        return String::new();
    };
    #[cfg(feature = "ast-context")]
    {
        return crate::ast_context::build_call_graph_context(path, files).await;
    }
    #[cfg(not(feature = "ast-context"))]
    {
        let _ = (path, files);
        String::new()
    }
}

/// Infers the repository path from the current working directory.
#[cfg(not(target_arch = "wasm32"))]
fn infer_repo_path_from_cwd(pr_owner: &str, pr_repo: &str) -> Option<String> {
    let git_root = get_git_root()?;
    let origin_url = get_git_origin_url()?;

    let Some((origin_owner, origin_repo)) = parse_origin_owner_repo(&origin_url) else {
        tracing::debug!(
            "infer_repo_path_from_cwd: parse_origin_owner_repo failed for {}",
            origin_url
        );
        return None;
    };

    let pr_owner_lower = pr_owner.to_lowercase();
    let pr_repo_lower = pr_repo.to_lowercase();

    if origin_owner == pr_owner_lower && origin_repo == pr_repo_lower {
        tracing::debug!(
            "infer_repo_path_from_cwd: matched origin {}/{} with PR {}/{}",
            origin_owner,
            origin_repo,
            pr_owner_lower,
            pr_repo_lower
        );
        Some(git_root)
    } else {
        tracing::debug!(
            "infer_repo_path_from_cwd: origin {}/{} does not match PR {}/{}",
            origin_owner,
            origin_repo,
            pr_owner_lower,
            pr_repo_lower
        );
        None
    }
}

/// Get git repository root directory.
#[cfg(not(target_arch = "wasm32"))]
fn get_git_root() -> Option<String> {
    use std::process::Command;

    Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}

/// Get git origin URL.
#[cfg(not(target_arch = "wasm32"))]
fn get_git_origin_url() -> Option<String> {
    use std::process::Command;

    Command::new("git")
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}

/// Parses git remote URL to extract owner and repo.
fn parse_origin_owner_repo(url: &str) -> Option<(String, String)> {
    use crate::utils::parse_git_remote_url;

    let Ok(parsed) = parse_git_remote_url(url) else {
        return None;
    };

    let parts: Vec<&str> = parsed.split('/').collect();
    if parts.len() != 2 {
        return None;
    }

    let owner = parts[0].to_lowercase();
    let repo = parts[1].to_lowercase();
    Some((owner, repo))
}
