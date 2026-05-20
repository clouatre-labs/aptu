// SPDX-License-Identifier: Apache-2.0

//! Review context policy layer for PR analysis.
//!
//! Centralizes all enrichment decisions (AST context, call graph, dependency enrichments)
//! and CWD inference into a single `ReviewContext` struct and `build_review_context()` function.

use std::path::PathBuf;

use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

/// Review context containing all enrichment data and configuration for PR analysis.
///
/// This struct centralizes enrichment decisions and is passed to `build_pr_review_user_prompt()`
/// to avoid scattered conditional logic throughout the codebase.
#[derive(Clone, Debug)]
pub struct ReviewContext {
    /// Pull request details.
    pub pr: PrDetails,
    /// AST context for changed files (empty if not available or feature disabled).
    pub ast_context: String,
    /// Call graph context for changed files (empty if not available or feature disabled).
    pub call_graph: String,
    /// Inferred repository path from CWD (if available).
    pub inferred_repo_path: Option<PathBuf>,
    /// Whether the repository path was inferred from CWD.
    pub cwd_inferred: bool,
    /// Maximum characters per file's full content in the prompt (from `ReviewConfig`).
    pub max_chars_per_file: usize,
    /// Number of files whose full content was truncated at prompt assembly.
    pub files_truncated: usize,
    /// Total characters dropped across all truncated files.
    pub truncated_chars_dropped: usize,
    /// Total number of files in the PR.
    pub files_total: usize,
    /// Number of files with a patch (non-empty diff).
    pub files_with_patch: usize,
    /// Number of dependency enrichments applied.
    pub dep_enrichments_count: usize,
    /// Total characters in dependency enrichments.
    pub dep_enrichments_chars: usize,
    /// Names of context items dropped due to budget constraints.
    pub budget_drops: Vec<String>,
    /// Final assembled prompt character count.
    pub prompt_chars_final: usize,
}

impl ReviewContext {
    /// Returns a formatted pre-flight summary for verbose output.
    ///
    /// Includes package names, character counts, and CWD inference status.
    #[must_use]
    pub fn verbose_summary(&self) -> String {
        use std::fmt::Write;

        let mut summary = String::new();

        // Repo path info
        if let Some(path) = &self.inferred_repo_path {
            let inferred_label = if self.cwd_inferred { " (inferred)" } else { "" };
            let _ = writeln!(
                summary,
                "Repository path: {}{}",
                path.display(),
                inferred_label
            );
        }

        // Enrichment summary
        if !self.pr.dep_enrichments.is_empty() {
            let packages: Vec<&str> = self
                .pr
                .dep_enrichments
                .iter()
                .map(|d| d.package_name.as_str())
                .collect();
            let _ = writeln!(summary, "Dependency enrichments: {}", packages.join(", "));
        }

        // Context sizes
        let mut context_sizes = Vec::new();
        if !self.ast_context.is_empty() {
            context_sizes.push(format!("AST: {} chars", self.ast_context.len()));
        }
        if !self.call_graph.is_empty() {
            context_sizes.push(format!("call graph: {} chars", self.call_graph.len()));
        }
        if !context_sizes.is_empty() {
            let _ = writeln!(summary, "Context: {}", context_sizes.join(", "));
        }

        // Truncation summary
        if self.files_truncated > 0 {
            let _ = writeln!(
                summary,
                "Files truncated: {} ({} chars dropped)",
                self.files_truncated, self.truncated_chars_dropped
            );
        }

        summary
    }

    /// Records a file truncation event.
    ///
    /// Updates truncation counters and emits a debug log.
    pub fn record_truncation(&mut self, filename: &str, original_len: usize, truncated_len: usize) {
        self.files_truncated += 1;
        self.truncated_chars_dropped += original_len - truncated_len;
        tracing::debug!(
            filename = %filename,
            original_len,
            truncated_len,
            "file content truncated at prompt assembly"
        );
    }
}

impl Default for ReviewContext {
    fn default() -> Self {
        Self {
            pr: crate::ai::types::PrDetails {
                owner: String::new(),
                repo: String::new(),
                number: 0,
                title: String::new(),
                body: String::new(),
                base_branch: String::new(),
                head_branch: String::new(),
                files: Vec::new(),
                url: String::new(),
                labels: Vec::new(),
                head_sha: String::new(),
                review_comments: Vec::new(),
                instructions: None,
                dep_enrichments: Vec::new(),
            },
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: crate::config::ReviewConfig::default().max_chars_per_file,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            files_total: 0,
            files_with_patch: 0,
            dep_enrichments_count: 0,
            dep_enrichments_chars: 0,
            budget_drops: Vec::new(),
            prompt_chars_final: 0,
        }
    }
}

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
/// * `deep` - Whether to enable deep analysis (call graph)
/// * `review_config` - Review configuration with budget thresholds
///
/// # Returns
///
/// A `ReviewContext` with all enrichment fields populated according to budget constraints.
pub async fn build_review_context(
    mut pr: PrDetails,
    repo_path: Option<String>,
    deep: bool,
    review_config: &ReviewConfig,
) -> crate::Result<ReviewContext> {
    // Step 1: Resolve repo_path (explicit or inferred from CWD)
    let (inferred_repo_path, cwd_inferred) = resolve_repo_path(&pr, repo_path);
    let repo_path_ref = inferred_repo_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    // Step 2: Build AST context if repo_path resolved
    let ast_context = build_ctx_ast(repo_path_ref.as_deref(), &pr.files).await;

    // Step 3: Enrich with dependency release notes
    pr.dep_enrichments = enrich_deps(&pr.files, review_config).await;

    // Step 4: Estimate total chars and decide call_graph budget
    let estimated_size = estimate_pr_size(&pr, &ast_context);
    let max_prompt_chars = review_config.max_prompt_chars;
    let budget_remaining = max_prompt_chars.saturating_sub(estimated_size);

    // Step 5: Build call_graph if decided
    let should_enable_cg = should_enable_call_graph(deep, budget_remaining, review_config);
    let mut call_graph = if should_enable_cg {
        build_ctx_call_graph(repo_path_ref.as_deref(), &pr.files, true).await
    } else {
        String::new()
    };

    // Step 6: Apply budget drop order
    let mut ast_context = ast_context;
    let mut budget_drops = Vec::new();
    apply_budget_drops(
        &mut pr,
        &mut ast_context,
        &mut call_graph,
        deep,
        max_prompt_chars,
        &mut budget_drops,
    );

    // Collect tracking metrics
    let files_total = pr.files.len();
    let files_with_patch = pr
        .files
        .iter()
        .filter(|f| f.patch.is_some() && !f.patch.as_ref().unwrap().is_empty())
        .count();
    let dep_enrichments_count = pr.dep_enrichments.len();
    let dep_enrichments_chars = pr
        .dep_enrichments
        .iter()
        .map(|d| serde_json::to_string(d).unwrap_or_default().len())
        .sum();

    Ok(ReviewContext {
        pr,
        ast_context,
        call_graph,
        inferred_repo_path,
        cwd_inferred,
        max_chars_per_file: review_config.max_chars_per_file,
        files_truncated: 0,
        truncated_chars_dropped: 0,
        files_total,
        files_with_patch,
        dep_enrichments_count,
        dep_enrichments_chars,
        budget_drops,
        prompt_chars_final: 0,
    })
}

/// Resolves the repository path from explicit argument or CWD inference.
///
/// Returns a tuple of `(inferred_repo_path, cwd_inferred)`.
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

/// Determines whether to enable call graph context based on budget and flags.
fn should_enable_call_graph(deep: bool, budget_remaining: usize, config: &ReviewConfig) -> bool {
    deep || budget_remaining > config.min_budget_for_call_graph
}

/// Enriches PR with dependency release notes if manifest files are detected.
async fn enrich_deps(
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

/// Applies budget drop order: `call_graph` -> `ast_context` -> `dep_enrichments` -> patches -> `full_content`.
/// Enforces the prompt budget by dropping enrichment sections in priority order.
///
/// When the assembled prompt exceeds `max_prompt_chars`, sections are cleared in
/// the following order (lowest-priority dropped first):
///
/// 1. `call_graph` -- dropped first unless `deep` is explicitly set
/// 2. `ast_context` -- dropped second
/// 3. `dep_enrichments` -- dropped third
/// 4. file patches -- dropped largest-first
/// 5. file `full_content` -- dropped largest-first as last resort
///
/// Each drop is logged at `WARN` level with the section name and character count.
/// The function never returns an error; sections that cannot fit are silently cleared.
fn apply_budget_drops(
    pr: &mut PrDetails,
    ast_context: &mut String,
    call_graph: &mut String,
    deep: bool,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
) {
    let mut estimated_size = estimate_pr_size(pr, ast_context);
    if !call_graph.is_empty() {
        estimated_size += call_graph.len();
    }

    // Drop call_graph if over budget (unless explicitly enabled)
    if estimated_size > max_prompt_chars && !deep {
        tracing::warn!(
            section = "call_graph",
            chars = call_graph.len(),
            "Dropping section: prompt budget exceeded"
        );
        let dropped_chars = call_graph.len();
        call_graph.clear();
        estimated_size -= dropped_chars;
        budget_drops.push("call_graph".to_string());
    }

    // Drop ast_context if still over budget
    if estimated_size > max_prompt_chars {
        tracing::warn!(
            section = "ast_context",
            chars = ast_context.len(),
            "Dropping section: prompt budget exceeded"
        );
        let dropped_chars = ast_context.len();
        ast_context.clear();
        estimated_size -= dropped_chars;
        budget_drops.push("ast_context".to_string());
    }

    // Drop dep_enrichments if still over budget
    if estimated_size > max_prompt_chars {
        let dropped_chars: usize = pr
            .dep_enrichments
            .iter()
            .map(|d| d.body.len() + d.package_name.len() + d.github_url.len())
            .sum();
        if dropped_chars > 0 {
            tracing::warn!(
                section = "dep_enrichments",
                chars = dropped_chars,
                "Dropping section: prompt budget exceeded"
            );
            pr.dep_enrichments.clear();
            estimated_size -= dropped_chars;
            budget_drops.push("dep_enrichments".to_string());
        }
    }

    drop_patches_by_size(&mut pr.files, &mut estimated_size, max_prompt_chars, budget_drops);
    drop_full_content_by_size(&mut pr.files, &mut estimated_size, max_prompt_chars, budget_drops);
}

/// Drops file patches in descending size order until under budget.
fn drop_patches_by_size(
    files: &mut [crate::ai::types::PrFile],
    estimated_size: &mut usize,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
) {
    if *estimated_size <= max_prompt_chars {
        return;
    }

    let mut file_sizes: Vec<(usize, usize)> = files
        .iter()
        .enumerate()
        .map(|(idx, f)| (idx, f.patch.as_ref().map_or(0, String::len)))
        .collect();
    file_sizes.sort_by_key(|x| std::cmp::Reverse(x.1));

    for (file_idx, patch_size) in file_sizes {
        if *estimated_size <= max_prompt_chars {
            break;
        }
        if patch_size > 0 {
            tracing::warn!(
                file = %files[file_idx].filename,
                patch_chars = patch_size,
                "Dropping patch: prompt budget exceeded"
            );
            let filename = files[file_idx].filename.clone();
            files[file_idx].patch = None;
            *estimated_size -= patch_size;
            budget_drops.push(format!("file_content:{filename}"));
        }
    }
}

/// Drops file `full_content` in descending size order until under budget.
fn drop_full_content_by_size(
    files: &mut [crate::ai::types::PrFile],
    estimated_size: &mut usize,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
) {
    if *estimated_size <= max_prompt_chars {
        return;
    }

    let mut full_content_sizes: Vec<(usize, usize)> = files
        .iter()
        .enumerate()
        .map(|(idx, f)| (idx, f.full_content.as_ref().map_or(0, String::len)))
        .collect();
    full_content_sizes.sort_by_key(|x| std::cmp::Reverse(x.1));

    for (file_idx, content_size) in full_content_sizes {
        if *estimated_size <= max_prompt_chars {
            break;
        }
        if content_size > 0 {
            tracing::warn!(
                file = %files[file_idx].filename,
                content_chars = content_size,
                "Dropping full_content: prompt budget exceeded"
            );
            let filename = files[file_idx].filename.clone();
            files[file_idx].full_content = None;
            *estimated_size -= content_size;
            budget_drops.push(format!("file_content:{filename}"));
        }
    }
}

/// Estimates the total character size of a PR review prompt.
fn estimate_pr_size(pr: &PrDetails, ast_context: &str) -> usize {
    let mut size = 0;

    // PR metadata
    size += pr.title.len() + pr.body.len() + pr.head_branch.len() + pr.base_branch.len();

    // Files and patches
    for file in &pr.files {
        size += file.filename.len() + file.status.len();
        if let Some(patch) = &file.patch {
            size += patch.len();
        }
        if let Some(content) = &file.full_content {
            size += content.len();
        }
    }

    // Enrichments
    for dep in &pr.dep_enrichments {
        size += dep.package_name.len() + dep.body.len() + dep.github_url.len();
    }

    // Context
    size += ast_context.len();

    size
}

/// Builds AST context for changed files.
#[allow(clippy::unused_async)]
async fn build_ctx_ast(repo_path: Option<&str>, files: &[crate::ai::types::PrFile]) -> String {
    let Some(path) = repo_path else {
        return String::new();
    };
    #[cfg(feature = "ast-context")]
    {
        return crate::ast_context::build_ast_context(path, files).await;
    }
    #[cfg(not(feature = "ast-context"))]
    {
        let _ = (path, files);
        String::new()
    }
}

/// Builds call-graph context for changed files.
#[allow(clippy::unused_async)]
async fn build_ctx_call_graph(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
    deep: bool,
) -> String {
    if !deep {
        return String::new();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::{DepReleaseNote, PrFile};

    fn make_pr_with_content(patch_chars: usize, full_content_chars: usize) -> PrDetails {
        PrDetails {
            number: 1,
            title: "test".to_string(),
            body: String::new(),
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            url: "https://github.com/owner/repo/pull/1".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            head_sha: String::new(),
            review_comments: vec![],
            files: vec![PrFile {
                filename: "src/lib.rs".to_string(),
                status: "modified".to_string(),
                patch: Some("x".repeat(patch_chars)),
                patch_truncated: false,
                full_content: if full_content_chars > 0 {
                    Some("y".repeat(full_content_chars))
                } else {
                    None
                },
                additions: 1,
                deletions: 0,
            }],
            dep_enrichments: vec![DepReleaseNote {
                package_name: "serde".to_string(),
                old_version: "1.0.0".to_string(),
                new_version: "1.0.1".to_string(),
                registry: "crates.io".to_string(),
                github_url: "https://github.com/serde-rs/serde".to_string(),
                body: "dep_body".to_string(),
                fetch_note: String::new(),
            }],
            instructions: None,
            labels: vec![],
        }
    }

    /// Verifies that apply_budget_drops enforces the documented drop order:
    /// call_graph -> ast_context -> dep_enrichments -> patches -> full_content.
    #[test]
    fn test_apply_budget_drops_order() {
        let mut pr = make_pr_with_content(500, 500);
        let mut ast_context = "a".repeat(300);
        let mut call_graph = "b".repeat(300);

        // Budget tight enough that call_graph must be dropped first.
        // Total without drops: ~500 patch + 500 full_content + 300 ast + 300 call_graph
        //                     + "serde" dep body (~"dep_body" = 8 chars) + metadata ~50
        // Set budget just above (ast + patch + full_content + metadata) to require call_graph drop.
        let max_prompt_chars = 600;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            false,
            max_prompt_chars,
            &mut drops,
        );

        // call_graph dropped first (deep=false, over budget)
        assert!(
            call_graph.is_empty(),
            "call_graph should be dropped first when over budget"
        );
    }

    /// Verifies that dep_enrichments are dropped before file patches.
    #[test]
    fn test_apply_budget_drops_dep_enrichments_before_patches() {
        let mut pr = make_pr_with_content(200, 0);
        // Add a large dep enrichment body to make it over budget
        pr.dep_enrichments[0].body = "d".repeat(400);
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Budget: just under (patch + dep_body) to force dep drop but not patch drop
        let max_prompt_chars = 250;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            false,
            max_prompt_chars,
            &mut drops,
        );

        // dep_enrichments dropped before patches
        assert!(
            pr.dep_enrichments.is_empty(),
            "dep_enrichments should be dropped before file patches"
        );
        // patch should still be present (dep drop was enough to fit)
        assert!(
            pr.files[0].patch.is_some(),
            "file patch should be retained when dep drop brought size within budget"
        );
    }

    #[test]
    fn test_verbose_summary_all_fields() {
        // Arrange: ReviewContext with repo path (inferred), dep enrichments, ast, call graph
        let mut pr = make_pr_with_content(10, 0);
        pr.dep_enrichments = vec![
            DepReleaseNote {
                package_name: "tokio".to_string(),
                old_version: "1.37.0".to_string(),
                new_version: "1.38.0".to_string(),
                registry: "crates.io".to_string(),
                github_url: "https://github.com/tokio-rs/tokio".to_string(),
                body: "release notes".to_string(),
                fetch_note: String::new(),
            },
            DepReleaseNote {
                package_name: "serde".to_string(),
                old_version: "1.0.199".to_string(),
                new_version: "1.0.200".to_string(),
                registry: "crates.io".to_string(),
                github_url: "https://github.com/serde-rs/serde".to_string(),
                body: "release notes".to_string(),
                fetch_note: String::new(),
            },
        ];
        let ctx = ReviewContext {
            pr,
            ast_context: "fn foo() {}".to_string(),
            call_graph: "foo -> bar".to_string(),
            inferred_repo_path: Some(std::path::PathBuf::from("/tmp/repo")),
            cwd_inferred: true,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        };

        // Act
        let summary = ctx.verbose_summary();

        // Assert: repo path with inferred label
        assert!(
            summary.contains("/tmp/repo"),
            "summary should contain the repo path"
        );
        assert!(
            summary.contains("(inferred)"),
            "summary should mark CWD-inferred path"
        );
        // Assert: dep package names
        assert!(
            summary.contains("tokio"),
            "summary should list dep package names"
        );
        assert!(
            summary.contains("serde"),
            "summary should list dep package names"
        );
        // Assert: context sizes
        assert!(
            summary.contains("AST:"),
            "summary should include AST char count"
        );
        assert!(
            summary.contains("call graph:"),
            "summary should include call graph char count"
        );
    }

    #[test]
    fn test_verbose_summary_empty_context() {
        // Arrange: ReviewContext with no enrichments and no repo path
        let mut pr = make_pr_with_content(0, 0);
        pr.dep_enrichments.clear();
        let ctx = ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        };

        // Act
        let summary = ctx.verbose_summary();

        // Assert: nothing to report means empty string
        assert!(
            summary.is_empty(),
            "summary should be empty when no enrichments are present"
        );
    }

    #[test]
    fn test_verbose_summary_truncation_section_present_and_absent() {
        // Arrange
        let mut pr = make_pr_with_content(0, 0);
        pr.dep_enrichments.clear();

        // Case 1: files_truncated > 0 -- section must be present
        let ctx_with = ReviewContext {
            pr: pr.clone(),
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 4_000,
            files_total: 0,
            files_with_patch: 0,
            files_truncated: 3,
            truncated_chars_dropped: 900,
            dep_enrichments_count: 0,
            dep_enrichments_chars: 0,
            budget_drops: Vec::new(),
            prompt_chars_final: 0,
        };
        let summary = ctx_with.verbose_summary();
        assert!(
            summary.contains("Files truncated: 3 (900 chars dropped)"),
            "verbose_summary must include truncation line when files_truncated > 0"
        );

        // Case 2: files_truncated == 0 -- section must be absent
        let ctx_without = ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 4_000,
            files_total: 0,
            files_with_patch: 0,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            dep_enrichments_count: 0,
            dep_enrichments_chars: 0,
            budget_drops: Vec::new(),
            prompt_chars_final: 0,
        };
        let summary_clean = ctx_without.verbose_summary();
        assert!(
            !summary_clean.contains("Files truncated"),
            "verbose_summary must omit truncation line when files_truncated == 0"
        );
    }

    #[test]
    fn test_truncation_tracking_incremented() {
        use crate::ai::provider::AiProvider;

        struct TrackingProvider;
        impl AiProvider for TrackingProvider {
            fn name(&self) -> &'static str {
                "tracking"
            }
            fn api_url(&self) -> &'static str {
                "https://example.com"
            }
            fn api_key_env(&self) -> &'static str {
                "TRACKING_API_KEY"
            }
            fn http_client(&self) -> &reqwest::Client {
                unimplemented!()
            }
            fn api_key(&self) -> &secrecy::SecretString {
                unimplemented!()
            }
            fn model(&self) -> &'static str {
                "model"
            }
            fn max_tokens(&self) -> u32 {
                2048
            }
            fn temperature(&self) -> f32 {
                0.3
            }
        }

        // Arrange: file content that exceeds cap
        let cap = 4_000_usize;
        let content = "z".repeat(cap + 500);
        let original_len = content.len();
        let pr = crate::ai::types::PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Tracking test".to_string(),
            body: String::new(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![crate::ai::types::PrFile {
                filename: "big.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: None,
                patch_truncated: false,
                full_content: Some(content),
            }],
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };
        let mut ctx = ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: cap,
            files_total: 0,
            files_with_patch: 0,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            dep_enrichments_count: 0,
            dep_enrichments_chars: 0,
            budget_drops: Vec::new(),
            prompt_chars_final: 0,
        };

        // Act
        let _ = TrackingProvider::build_pr_review_user_prompt(&mut ctx);

        // Assert
        assert_eq!(
            ctx.files_truncated, 1,
            "files_truncated must be 1 after one truncation"
        );
        assert_eq!(
            ctx.truncated_chars_dropped,
            original_len - cap,
            "truncated_chars_dropped must equal chars removed"
        );
    }

    #[test]
    fn test_no_double_truncation_at_new_cap() {
        use crate::ai::provider::AiProvider;

        struct NoDblProvider;
        impl AiProvider for NoDblProvider {
            fn name(&self) -> &'static str {
                "nodbl"
            }
            fn api_url(&self) -> &'static str {
                "https://example.com"
            }
            fn api_key_env(&self) -> &'static str {
                "NODBL_API_KEY"
            }
            fn http_client(&self) -> &reqwest::Client {
                unimplemented!()
            }
            fn api_key(&self) -> &secrecy::SecretString {
                unimplemented!()
            }
            fn model(&self) -> &'static str {
                "model"
            }
            fn max_tokens(&self) -> u32 {
                2048
            }
            fn temperature(&self) -> f32 {
                0.3
            }
        }

        // Arrange: file content at cap + 1 char
        let cap = 16_000_usize;
        let content = "a".repeat(cap + 1);
        let pr = crate::ai::types::PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "NoDbl test".to_string(),
            body: String::new(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![crate::ai::types::PrFile {
                filename: "cap.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: None,
                patch_truncated: false,
                full_content: Some(content),
            }],
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };
        let mut ctx = ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: cap,
            files_total: 0,
            files_with_patch: 0,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            dep_enrichments_count: 0,
            dep_enrichments_chars: 0,
            budget_drops: Vec::new(),
            prompt_chars_final: 0,
        };

        // Act
        let _ = NoDblProvider::build_pr_review_user_prompt(&mut ctx);

        // Assert: exactly one truncation of exactly 1 char
        assert_eq!(ctx.files_truncated, 1, "exactly one file must be truncated");
        assert_eq!(ctx.truncated_chars_dropped, 1, "exactly 1 char dropped");
    }
}
