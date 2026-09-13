// SPDX-License-Identifier: Apache-2.0

//! Review context policy layer for PR analysis.
//!
//! Centralizes all enrichment decisions (AST context, call graph, dependency enrichments)
//! and CWD inference into a single `ReviewContext` struct and `build_review_context()` function.

use std::path::PathBuf;

use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

#[cfg(feature = "ast-context")]
use std::fmt::Write as _;

#[cfg(feature = "ast-context")]
use aptu_coder_core::{analyze_file, language_for_extension};

/// Estimated overhead for XML tags, section headers, and schema preamble added by
/// `build_pr_review_user_prompt`. Used to ensure the prompt budget accounts for
/// non-content characters when estimating total prompt size.
pub(crate) const PROMPT_OVERHEAD_CHARS: usize = 1_000;

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
    /// Symbol expansions for changed symbols with an unambiguous out-of-diff caller
    /// (empty unless `deep` is explicitly requested; see `build_ctx_symbol_expansions`).
    pub symbol_expansions: Vec<crate::ai::types::SymbolExpansion>,
    /// Inferred repository path from CWD (if available).
    pub inferred_repo_path: Option<PathBuf>,
    /// Whether the repository path was inferred from CWD.
    pub cwd_inferred: bool,
    /// Maximum characters per file's full content in the prompt (from `ReviewConfig`).
    pub max_chars_per_file: usize,
    /// Maximum total diff characters across all files in the prompt (from `ReviewConfig`).
    pub max_diff_chars: usize,
    /// Maximum characters per individual file patch before the patch is dropped entirely (from `ReviewConfig`).
    pub max_patch_chars_per_file: usize,
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
    /// Estimated total character size of the PR review prompt before budget drops.
    pub estimated_size: usize,
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
            symbol_expansions: Vec::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: crate::config::ReviewConfig::default().max_chars_per_file,
            max_diff_chars: crate::config::ReviewConfig::default().max_diff_chars,
            max_patch_chars_per_file: crate::config::ReviewConfig::default()
                .max_patch_chars_per_file,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            files_total: 0,
            files_with_patch: 0,
            dep_enrichments_count: 0,
            dep_enrichments_chars: 0,
            budget_drops: Vec::new(),
            prompt_chars_final: 0,
            estimated_size: 0,
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
    // (call_graph and symbol_expansions not yet built, pass empty)
    let estimated_size = estimate_pr_size(&pr, &ast_context, "", &[]);
    let max_prompt_chars = review_config.max_prompt_chars;
    let budget_remaining = max_prompt_chars.saturating_sub(estimated_size);

    // Step 5: Build call_graph if decided
    let should_enable_cg = should_enable_call_graph(deep, budget_remaining, review_config);
    let mut call_graph = if should_enable_cg {
        build_ctx_call_graph(repo_path_ref.as_deref(), &pr.files, true).await
    } else {
        String::new()
    };

    // Step 5b: Build symbol_expansions gated on `deep` only -- unlike call_graph, this
    // is not auto-enabled via the budget-remaining heuristic, since adoption of this
    // feature is pending a preregistered benchmark.
    let mut symbol_expansions = build_ctx_symbol_expansions(
        repo_path_ref.as_deref(),
        &pr.files,
        deep,
        review_config.max_symbol_expansion_chars,
    )
    .await;

    // Re-estimate with actual call_graph and symbol_expansions for accurate routing
    let final_estimated_size = estimate_pr_size(&pr, &ast_context, &call_graph, &symbol_expansions);

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
        &mut symbol_expansions,
        deep,
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
        symbol_expansions,
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

/// Applies budget drop order: `call_graph` -> `ast_context` -> `symbol_expansions` ->
/// `dep_enrichments` -> `full_content` -> patches.
/// Enforces the prompt budget by dropping enrichment sections in priority order.
///
/// When the assembled prompt exceeds `max_prompt_chars`, sections are cleared in
/// the following order (lowest-priority dropped first):
///
/// 1. `call_graph` -- dropped first unless `deep` is explicitly set
/// 2. `ast_context` -- dropped second
/// 3. `symbol_expansions` -- dropped third
/// 4. `dep_enrichments` -- dropped fourth
/// 5. file `full_content` -- dropped largest-first
/// 6. file patches -- dropped largest-first as last resort, so the diff itself
///    (the highest-value context for a review) is preserved as long as possible
///
/// Each drop is logged at `WARN` level with the section name and character count.
/// The function never returns an error; sections that cannot fit are silently cleared.
#[allow(clippy::too_many_arguments)]
fn apply_budget_drops(
    pr: &mut PrDetails,
    ast_context: &mut String,
    call_graph: &mut String,
    symbol_expansions: &mut Vec<crate::ai::types::SymbolExpansion>,
    deep: bool,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
    repo_path: Option<&str>,
) {
    let mut estimated_size = estimate_pr_size(pr, ast_context, call_graph, symbol_expansions);

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
        if dropped_chars > 0 {
            budget_drops.push("call_graph".to_string());
        }
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
        if dropped_chars > 0 {
            budget_drops.push("ast_context".to_string());
        }
    }

    // Drop symbol_expansions if still over budget
    if estimated_size > max_prompt_chars {
        let dropped_chars: usize = symbol_expansions
            .iter()
            .map(|e| e.snippet.len() + e.reference_path.len())
            .sum();
        if dropped_chars > 0 {
            tracing::warn!(
                section = "symbol_expansions",
                chars = dropped_chars,
                "Dropping section: prompt budget exceeded"
            );
            symbol_expansions.clear();
            estimated_size -= dropped_chars;
            budget_drops.push("symbol_expansions".to_string());
        }
    }

    drop_dep_enrichments_by_size(pr, &mut estimated_size, max_prompt_chars, budget_drops);

    drop_full_content_by_size(
        &mut pr.files,
        &mut estimated_size,
        max_prompt_chars,
        budget_drops,
        repo_path,
    );
    drop_patches_by_size(
        &mut pr.files,
        &mut estimated_size,
        max_prompt_chars,
        budget_drops,
    );
}

/// Drops `dep_enrichments` if the prompt is still over budget.
fn drop_dep_enrichments_by_size(
    pr: &mut PrDetails,
    estimated_size: &mut usize,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
) {
    if *estimated_size <= max_prompt_chars {
        return;
    }
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
        *estimated_size -= dropped_chars;
        budget_drops.push("dep_enrichments".to_string());
    }
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
            budget_drops.push(format!("patch:{filename}"));
        }
    }
}

/// Builds a compact signature-outline block for a single file, mirroring the
/// per-function signature and imports summary used by `ast_context.rs`.
///
/// Returns `None` when the `ast-context` feature is disabled, the file's
/// extension is unsupported, or analysis fails.
#[cfg(feature = "ast-context")]
fn build_file_outline(repo_path: &str, filename: &str) -> Option<String> {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if language_for_extension(ext).is_none() {
        tracing::debug!("build_file_outline: unsupported extension for {filename}: {ext:?}");
        return None;
    }
    let full_path = std::path::Path::new(repo_path).join(filename);
    let path_str = full_path.to_string_lossy().into_owned();

    match analyze_file(&path_str, None) {
        Ok(analysis) => {
            let mut outline = format!("## {filename}\n");
            for func in &analysis.semantic.functions {
                let _ = writeln!(outline, "  fn {}", func.compact_signature());
            }
            if !analysis.semantic.imports.is_empty() {
                outline.push_str("  imports:");
                for imp in analysis.semantic.imports.iter().take(5) {
                    let _ = write!(outline, " {}", imp.module);
                }
                outline.push('\n');
            }
            Some(outline)
        }
        Err(e) => {
            tracing::debug!("build_file_outline: skipping {filename}: {e}");
            None
        }
    }
}

/// Stub used when the `ast-context` feature is disabled; always returns `None`
/// so callers fall through to the existing full-clear behavior.
#[cfg(not(feature = "ast-context"))]
fn build_file_outline(_repo_path: &str, _filename: &str) -> Option<String> {
    None
}

/// Drops file `full_content` in descending size order until under budget.
///
/// Before fully clearing a file's `full_content`, attempts to substitute a
/// compact signature outline (via `build_file_outline`) when `repo_path` is
/// available and the outline actually brings the prompt back under budget.
/// Falls back to the existing full-clear behavior otherwise.
///
/// The `outline_len < content_size` check is load-bearing, not cosmetic: it
/// guarantees the substitution never grows the prompt relative to the
/// full-clear fallback it replaces.
fn drop_full_content_by_size(
    files: &mut [crate::ai::types::PrFile],
    estimated_size: &mut usize,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
    repo_path: Option<&str>,
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
        if content_size == 0 {
            continue;
        }

        let filename = files[file_idx].filename.clone();

        if let Some(repo) = repo_path
            && let Some(outline) = build_file_outline(repo, &filename)
        {
            let outline_len = outline.len();
            let candidate_size = *estimated_size - content_size + outline_len;
            if outline_len < content_size && candidate_size <= max_prompt_chars {
                tracing::warn!(
                    file = %filename,
                    action = "outline_substituted",
                    original_chars = content_size,
                    outline_chars = outline_len,
                    "Substituting full_content with outline: prompt budget exceeded"
                );
                files[file_idx].full_content = Some(outline);
                *estimated_size = candidate_size;
                budget_drops.push(format!("file_content_outline:{filename}"));
                continue;
            }
        }

        tracing::warn!(
            file = %filename,
            action = "full_content_cleared",
            content_chars = content_size,
            "Dropping full_content: prompt budget exceeded"
        );
        files[file_idx].full_content = None;
        *estimated_size -= content_size;
        budget_drops.push(format!("file_content:{filename}"));
    }
}

/// Estimates the total character size of a PR review prompt.
///
/// Sums title, body, file metadata, patches, `full_content`, `dep_enrichments`,
/// `ast_context`, `call_graph`, `symbol_expansions`, and overhead.
#[must_use]
pub(crate) fn estimate_pr_size(
    pr: &PrDetails,
    ast_context: &str,
    call_graph: &str,
    symbol_expansions: &[crate::ai::types::SymbolExpansion],
) -> usize {
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

    // Call graph
    size += call_graph.len();

    // Symbol expansions
    for expansion in symbol_expansions {
        size += expansion.snippet.len() + expansion.reference_path.len();
    }

    // Overhead
    size += PROMPT_OVERHEAD_CHARS;

    size
}

/// Builds AST context for changed files.
///
/// Returns an [`AstContextOutput`] containing the text string built from analysis.
#[allow(clippy::unused_async)]
#[cfg(feature = "ast-context")]
async fn build_ctx_ast(
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
async fn build_ctx_ast(repo_path: Option<&str>, files: &[crate::ai::types::PrFile]) -> String {
    let _ = (repo_path, files);
    String::new()
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

/// Builds symbol-expansion context for changed symbols with exactly one
/// unambiguous out-of-diff caller.
///
/// Gated behind `deep` only -- unlike `build_ctx_call_graph`, this does not
/// auto-enable via the budget-remaining heuristic, since adoption is pending
/// a preregistered benchmark.
#[allow(clippy::unused_async)]
async fn build_ctx_symbol_expansions(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
    deep: bool,
    max_chars: usize,
) -> Vec<crate::ai::types::SymbolExpansion> {
    if !deep {
        return Vec::new();
    }
    let Some(path) = repo_path else {
        return Vec::new();
    };
    #[cfg(feature = "ast-context")]
    {
        return crate::ast_context::build_symbol_expansions_context(path, files, max_chars).await;
    }
    #[cfg(not(feature = "ast-context"))]
    {
        let _ = (path, files, max_chars);
        Vec::new()
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

/// Truncates `content` to at most `max_chars` characters, landing on the last newline
/// before the limit. Falls back to a char-boundary slice if no newline is found.
///
/// Returns the original content unchanged when its character count is already
/// within the limit.
#[must_use]
pub(crate) fn truncate_at_line_boundary(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }

    // Find the byte index of the max_chars-th character.
    let cutoff_byte = content
        .char_indices()
        .nth(max_chars)
        .map_or(content.len(), |(i, _)| i);

    // Scan backward from the cutoff byte to find the last newline.
    let truncated = &content[..cutoff_byte];
    if let Some(newline_pos) = truncated.rfind('\n') {
        content[..=newline_pos].to_string()
    } else {
        // No newline found; fall back to char-boundary slice at max_chars.
        content[..cutoff_byte].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::{DepReleaseNote, PrFile, SymbolExpansion};

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
            dep_enrichments: vec![],
            instructions: None,
            labels: vec![],
        }
    }

    fn make_dep(package_name: &str) -> DepReleaseNote {
        DepReleaseNote {
            package_name: package_name.to_string(),
            old_version: "1.0.0".to_string(),
            new_version: "1.0.1".to_string(),
            registry: "crates.io".to_string(),
            github_url: format!("https://github.com/owner/{package_name}"),
            body: "release notes".to_string(),
            fetch_note: String::new(),
        }
    }

    /// Verifies that `apply_budget_drops` enforces the documented drop order:
    /// `call_graph` -> `ast_context` -> `dep_enrichments` -> `patches` -> `full_content`.
    #[test]
    fn test_apply_budget_drops_order() {
        let mut pr = make_pr_with_content(500, 500);
        let mut ast_context = "a".repeat(300);
        let mut call_graph = "b".repeat(300);

        // Budget tight enough that call_graph must be dropped first.
        // Total with all: patch(500) + full_content(500) + ast(300) + call_graph(300)
        //                + metadata(~30) + PROMPT_OVERHEAD_CHARS(1000) = ~2630
        // Set budget to force call_graph drop (not deep).
        let max_prompt_chars = 600;

        let mut drops = Vec::new();
        // Priority order: call_graph -> ast_context -> dep_enrichments -> patches -> full_content
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut Vec::new(),
            false,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // call_graph dropped first (deep=false, over budget)
        assert!(
            call_graph.is_empty(),
            "call_graph should be dropped first when over budget"
        );
    }

    /// Verifies that `dep_enrichments` are dropped before file patches.
    #[test]
    fn test_apply_budget_drops_dep_enrichments_before_patches() {
        let mut pr = make_pr_with_content(200, 0);
        // Add a large dep enrichment body to make it over budget
        pr.dep_enrichments.push(make_dep("serde"));
        pr.dep_enrichments[0].body = "d".repeat(400);
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Budget: just under (patch + dep_body + overhead) to force dep drop but not patch drop.
        // Base estimate (without dep): patch(200) + metadata(~30) + PROMPT_OVERHEAD_CHARS(1000) = ~1230
        // With dep: + package_name(6) + body(400) + github_url(~30) = ~1666
        // Budget between 1230 and 1666 so dep is dropped but patch is retained.
        let max_prompt_chars = 1400;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut Vec::new(),
            false,
            max_prompt_chars,
            &mut drops,
            None,
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

    /// Verifies that empty sections do NOT appear in `budget_drops` telemetry.
    /// Regression test for telemetry accuracy bug where empty-section drops were unconditionally recorded.
    #[test]
    fn test_apply_budget_drops_empty_sections() {
        let mut pr = make_pr_with_content(100, 100);
        // All context sections are empty -- no content to drop
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Tight budget to trigger drop attempts, but sections are empty
        let max_prompt_chars = 500;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut Vec::new(),
            false,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // Empty sections should NOT appear in budget_drops
        // (they were cleared but no content was actually dropped)
        assert!(
            !drops.contains(&"call_graph".to_string()),
            "empty call_graph should not appear in budget_drops"
        );
        assert!(
            !drops.contains(&"ast_context".to_string()),
            "empty ast_context should not appear in budget_drops"
        );
    }

    /// Verifies that populated sections DO appear in `budget_drops` telemetry.
    /// Ensures the fix does not suppress telemetry for sections with actual content.
    #[test]
    fn test_apply_budget_drops_populated_sections() {
        let mut pr = make_pr_with_content(100, 100);
        // Populate all context sections with meaningful content
        let mut ast_context = "a".repeat(300);
        let mut call_graph = "b".repeat(300);

        // Tight budget to force drops, with populated sections
        let max_prompt_chars = 600;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut Vec::new(),
            false,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // Populated sections should appear in budget_drops (in priority order)
        // Priority order: call_graph -> ast_context
        assert!(
            drops.contains(&"call_graph".to_string()),
            "populated call_graph should appear in budget_drops"
        );
        // ast_context presence depends on budget constraints,
        // but at least call_graph must be present for this test
    }

    /// Regression test for issue #1548: verifies patch and `full_content` budget drops
    /// use distinct prefixes ("patch:" vs "`file_content`:") to avoid collision.
    #[test]
    fn test_budget_drops_distinguish_patch_from_full_content() {
        let mut pr = make_pr_with_content(500, 500);
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Budget tight enough to force BOTH patch and full_content drops for the file
        let max_prompt_chars = 50;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut Vec::new(),
            false,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // Both patch and full_content should be None (both dropped)
        assert!(
            pr.files[0].patch.is_none(),
            "patch should be dropped when over budget"
        );
        assert!(
            pr.files[0].full_content.is_none(),
            "full_content should be dropped when over budget"
        );

        // Drops should contain exactly two distinct entries: one for patch, one for full_content
        assert!(
            drops.contains(&"patch:src/lib.rs".to_string()),
            "budget_drops should contain 'patch:src/lib.rs' for the dropped patch"
        );
        assert!(
            drops.contains(&"file_content:src/lib.rs".to_string()),
            "budget_drops should contain 'file_content:src/lib.rs' for the dropped full_content"
        );

        // Verify they are distinct (not duplicates with the same prefix)
        let patch_drops: Vec<_> = drops.iter().filter(|s| s.starts_with("patch:")).collect();
        let full_content_drops: Vec<_> = drops
            .iter()
            .filter(|s| s.starts_with("file_content:"))
            .collect();
        assert_eq!(
            patch_drops.len(),
            1,
            "should have exactly one 'patch:' entry"
        );
        assert_eq!(
            full_content_drops.len(),
            1,
            "should have exactly one 'file_content:' entry"
        );
    }

    /// Creates a temp directory containing a single fixture file, for tests that
    /// need `analyze_file` to run against a real file on disk.
    #[cfg(feature = "ast-context")]
    fn make_outline_fixture(filename: &str, source: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        std::fs::write(dir.path().join(filename), source).expect("failed to write fixture file");
        dir
    }

    /// Verifies that when the signature outline fits within the remaining budget,
    /// `drop_full_content_by_size` substitutes it into `full_content` and records
    /// the `file_content_outline:` label instead of clearing the file entirely.
    #[cfg(feature = "ast-context")]
    #[test]
    fn test_drop_full_content_by_size_outline_fits_budget() {
        let dir = make_outline_fixture(
            "fixture.rs",
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        );
        let repo_path = dir.path().to_string_lossy().into_owned();

        let mut files = vec![crate::ai::types::PrFile {
            filename: "fixture.rs".to_string(),
            status: "modified".to_string(),
            patch: None,
            patch_truncated: false,
            full_content: Some("y".repeat(2000)),
            additions: 1,
            deletions: 0,
        }];
        let mut estimated_size = 2000;
        let max_prompt_chars = 100;
        let mut drops = Vec::new();

        drop_full_content_by_size(
            &mut files,
            &mut estimated_size,
            max_prompt_chars,
            &mut drops,
            Some(&repo_path),
        );

        assert!(
            files[0]
                .full_content
                .as_deref()
                .is_some_and(|c| c.contains("fn add")),
            "full_content should be replaced with a signature outline"
        );
        assert!(
            drops.contains(&"file_content_outline:fixture.rs".to_string()),
            "budget_drops should record the outline substitution"
        );
    }

    /// Verifies that when the outline itself still exceeds the remaining budget,
    /// `drop_full_content_by_size` falls back to fully clearing `full_content`
    /// without panicking.
    #[test]
    fn test_drop_full_content_by_size_outline_still_exceeds_budget() {
        let mut files = vec![crate::ai::types::PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: None,
            patch_truncated: false,
            full_content: Some("y".repeat(2000)),
            additions: 1,
            deletions: 0,
        }];
        let mut estimated_size = 2000;
        // A zero budget can never be satisfied by any non-empty outline, so this
        // exercises the fallback regardless of whether the ast-context feature
        // is enabled.
        let max_prompt_chars = 0;
        let mut drops = Vec::new();

        drop_full_content_by_size(
            &mut files,
            &mut estimated_size,
            max_prompt_chars,
            &mut drops,
            Some("/nonexistent/repo/path"),
        );

        assert!(
            files[0].full_content.is_none(),
            "full_content should be fully cleared when the outline cannot fit"
        );
        assert!(
            drops.contains(&"file_content:src/lib.rs".to_string()),
            "budget_drops should record the full clear, not an outline substitution"
        );
    }

    /// Verifies that an unsupported file extension skips `analyze_file` entirely
    /// and falls back to fully clearing `full_content`.
    #[test]
    fn test_drop_full_content_by_size_unsupported_extension_falls_back() {
        let mut files = vec![crate::ai::types::PrFile {
            filename: "data.unsupportedext".to_string(),
            status: "modified".to_string(),
            patch: None,
            patch_truncated: false,
            full_content: Some("y".repeat(200)),
            additions: 1,
            deletions: 0,
        }];
        let mut estimated_size = 200;
        let max_prompt_chars = 50;
        let mut drops = Vec::new();

        // repo_path points at a directory that does not exist; if analyze_file were
        // called, it would error, but the unsupported extension must short-circuit
        // before that call happens.
        drop_full_content_by_size(
            &mut files,
            &mut estimated_size,
            max_prompt_chars,
            &mut drops,
            Some("/nonexistent/repo/path"),
        );

        assert!(
            files[0].full_content.is_none(),
            "full_content should be cleared for unsupported extensions"
        );
        assert!(
            drops.contains(&"file_content:data.unsupportedext".to_string()),
            "budget_drops should record the full clear for the unsupported extension"
        );
    }

    /// Automated benchmark for issue #1592: measures the outline's actual size
    /// reduction against this crate's own real source files (not synthetic
    /// fixtures), confirming the substitution meaningfully shrinks prompt size
    /// on typical Rust files with real function counts and import lists.
    #[cfg(feature = "ast-context")]
    #[test]
    fn test_outline_size_reduction_on_real_source_files() {
        let repo_path = env!("CARGO_MANIFEST_DIR");
        let candidates = [
            "src/ai/review_context.rs",
            "src/ast_context.rs",
            "src/ai/types.rs",
        ];

        let mut measured = 0;
        for filename in candidates {
            let full_path = std::path::Path::new(repo_path).join(filename);
            let Ok(full_content) = std::fs::read_to_string(&full_path) else {
                continue;
            };
            let Some(outline) = build_file_outline(repo_path, filename) else {
                continue;
            };

            let reduction_pct = 100 - (outline.len() * 100 / full_content.len().max(1));
            println!(
                "{filename}: full_content={} chars, outline={} chars, reduction={reduction_pct}%",
                full_content.len(),
                outline.len()
            );

            assert!(
                outline.len() < full_content.len() / 2,
                "{filename}: outline ({} chars) should be well under half of full content ({} chars)",
                outline.len(),
                full_content.len()
            );
            measured += 1;
        }

        assert!(
            measured > 0,
            "expected at least one real source file to produce a measurable outline"
        );
    }

    /// Regression test for issue #1596: `full_content` must be evicted before patches,
    /// so a PR with a small diff but a large `full_content` still keeps its patch.
    #[test]
    fn test_apply_budget_drops_full_content_evicted_before_patch() {
        // patch is small, full_content alone is large enough to blow the budget.
        let mut pr = make_pr_with_content(50, 1000);
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Base without full_content: patch(50) + metadata(~30) + overhead(1000) = ~1080.
        // With full_content: + 1000 = ~2080. Budget sits between the two, so only
        // full_content needs dropping to fit; the patch must survive untouched.
        let max_prompt_chars = 1500;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut Vec::new(),
            false,
            max_prompt_chars,
            &mut drops,
            None,
        );

        assert!(
            pr.files[0].full_content.is_none(),
            "full_content should be evicted first when over budget"
        );
        assert!(
            pr.files[0].patch.is_some(),
            "patch should survive when evicting full_content alone brings the prompt under budget"
        );
        assert!(
            drops.contains(&"file_content:src/lib.rs".to_string()),
            "budget_drops should record the file_content eviction"
        );
        assert!(
            !drops.contains(&"patch:src/lib.rs".to_string()),
            "budget_drops should not record a patch eviction"
        );
    }

    /// Regression test for issue #1596 review feedback: when both `full_content` and
    /// patches must be dropped across multiple files, `budget_drops` must record every
    /// `file_content:` entry before any `patch:` entry, matching the call order.
    #[test]
    fn test_apply_budget_drops_records_full_content_before_patch_order() {
        let mut pr = make_pr_with_content(500, 100);
        pr.files.push(PrFile {
            filename: "src/b.rs".to_string(),
            status: "modified".to_string(),
            patch: Some("x".repeat(500)),
            patch_truncated: false,
            full_content: Some("y".repeat(100)),
            additions: 1,
            deletions: 0,
        });
        let mut ast_context = String::new();
        let mut call_graph = String::new();
        let max_prompt_chars = 1900;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut Vec::new(),
            false,
            max_prompt_chars,
            &mut drops,
            None,
        );

        let last_file_content_idx = drops.iter().rposition(|d| d.starts_with("file_content:"));
        let first_patch_idx = drops.iter().position(|d| d.starts_with("patch:"));
        assert!(
            last_file_content_idx.is_some() && first_patch_idx.is_some(),
            "expected both file_content and patch drops in this scenario: {drops:?}"
        );
        assert!(
            last_file_content_idx < first_patch_idx,
            "every file_content: entry must precede every patch: entry in budget_drops: {drops:?}"
        );
    }

    #[test]
    fn test_verbose_summary_all_fields() {
        // Arrange: ReviewContext with repo path (inferred), dep enrichments, ast, call graph
        let mut pr = make_pr_with_content(10, 0);
        pr.dep_enrichments = vec![make_dep("tokio"), make_dep("serde")];
        let ctx = ReviewContext {
            pr,
            ast_context: "fn foo() {}".to_string(),
            call_graph: "foo -> bar".to_string(),
            inferred_repo_path: Some(std::path::PathBuf::from("/tmp/repo")),
            cwd_inferred: true,
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
        let pr = make_pr_with_content(0, 0);
        let ctx = ReviewContext {
            pr,
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
        let pr = make_pr_with_content(0, 0);

        // Case 1: files_truncated > 0 -- section must be present
        let ctx_with = ReviewContext {
            pr: pr.clone(),
            max_chars_per_file: 4_000,
            files_truncated: 3,
            truncated_chars_dropped: 900,
            ..Default::default()
        };
        let summary = ctx_with.verbose_summary();
        assert!(
            summary.contains("Files truncated: 3 (900 chars dropped)"),
            "verbose_summary must include truncation line when files_truncated > 0"
        );

        // Case 2: files_truncated == 0 -- section must be absent
        let ctx_without = ReviewContext {
            pr,
            max_chars_per_file: 4_000,
            ..Default::default()
        };
        let summary_clean = ctx_without.verbose_summary();
        assert!(
            !summary_clean.contains("Files truncated"),
            "verbose_summary must omit truncation line when files_truncated == 0"
        );
    }

    #[test]
    fn test_should_enable_call_graph_budget_boundary() {
        // budget_remaining == min_budget_for_call_graph -> false (strict >)
        let config = ReviewConfig {
            min_budget_for_call_graph: 20_000,
            ..ReviewConfig::default()
        };
        assert!(
            !should_enable_call_graph(false, 20_000, &config),
            "should_enable_call_graph must be false when budget_remaining equals min_budget_for_call_graph"
        );
    }

    #[test]
    fn test_should_enable_call_graph_budget_below_threshold() {
        // budget_remaining < min_budget_for_call_graph, deep=false -> false
        let config = ReviewConfig {
            min_budget_for_call_graph: 20_000,
            ..ReviewConfig::default()
        };
        assert!(
            !should_enable_call_graph(false, 10_000, &config),
            "should_enable_call_graph must be false when budget_remaining < min_budget_for_call_graph and deep=false"
        );
    }

    #[test]
    fn test_should_enable_call_graph_deep_overrides_budget() {
        // deep=true bypasses the budget gate entirely
        let config = ReviewConfig {
            min_budget_for_call_graph: 20_000,
            ..ReviewConfig::default()
        };
        assert!(
            should_enable_call_graph(true, 0, &config),
            "should_enable_call_graph must be true when deep=true regardless of budget_remaining"
        );
    }

    // -----------------------------------------------------------------------
    // truncate_at_line_boundary tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_truncate_at_line_boundary_happy_path() {
        // Content with newlines; truncation should land on last newline before limit
        let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        // max_chars=20: chars "line 1\nline 2\nlin" -> find last '\n' -> "line 1\nline 2\n"
        let result = truncate_at_line_boundary(content, 20);
        assert_eq!(result, "line 1\nline 2\n");
        assert!(
            result.chars().count() <= 20,
            "truncated result must not exceed max_chars"
        );
        assert!(
            result.ends_with('\n'),
            "truncation should end at newline boundary when one exists"
        );
    }

    #[test]
    fn test_truncate_at_line_boundary_fallback_no_newline() {
        // Content with no newline; must fall back to char boundary
        let content = "abcdefghijklmnopqrstuvwxyz";
        let result = truncate_at_line_boundary(content, 10);
        assert_eq!(result, "abcdefghij");
        assert_eq!(result.chars().count(), 10);
    }

    #[test]
    fn test_truncate_at_line_boundary_under_limit() {
        // Content within limit should be returned unchanged
        let content = "short";
        let result = truncate_at_line_boundary(content, 100);
        assert_eq!(result, "short");
        assert_eq!(result.chars().count(), 5);
    }

    #[test]
    fn test_truncate_at_line_boundary_multi_byte_utf8() {
        // Multi-byte UTF-8 characters before the cut; must not panic
        let content: String = (0..30).map(|_| "\u{1F600}").collect(); // 30 emoji chars
        let result = truncate_at_line_boundary(&content, 25);
        // 25 chars from 30 should give 25 emoji chars (no newline, so char boundary fallback)
        assert_eq!(result.chars().count(), 25);
        // Every char should be the emoji
        assert!(result.chars().all(|c| c == '\u{1F600}'));
    }

    #[test]
    fn test_estimate_pr_size_includes_call_graph() {
        // Verify estimate_pr_size includes call_graph chars and PROMPT_OVERHEAD_CHARS
        let pr = make_pr_with_content(0, 0);
        let ast_context = "";
        let call_graph = "fn foo() -> bar\nfn baz() -> qux";
        let size = estimate_pr_size(&pr, ast_context, call_graph, &[]);
        let without_call_graph = estimate_pr_size(&pr, ast_context, "", &[]);
        // Delta between with and without call_graph should be exactly call_graph.len()
        assert_eq!(size - without_call_graph, call_graph.len());
        // Total should include PROMPT_OVERHEAD_CHARS
        assert!(size >= PROMPT_OVERHEAD_CHARS);
    }

    #[test]
    fn test_build_review_context_estimated_size_pre_budget() {
        // Verify estimate_pr_size accounts for call_graph + overhead before budget drops
        // using a non-minimal PrDetails with patches and full_content
        let pr = make_pr_with_content(50, 100);
        let ast_context = "fn foo() {}";
        let call_graph = "caller -> callee\nother -> thing";
        let size = estimate_pr_size(&pr, ast_context, call_graph, &[]);
        assert!(
            size >= call_graph.len() + PROMPT_OVERHEAD_CHARS,
            "estimated size {} should be >= call_graph.len() {} + overhead {}",
            size,
            call_graph.len(),
            PROMPT_OVERHEAD_CHARS
        );
    }

    /// Regression test for #1571 (structural graph removal): `build_review_context()`
    /// no longer threads a `GraphConfig` or builds `graph_context`/`graph_cache_hit`, but
    /// its `ast_context` and `call_graph` enrichment paths must be completely unaffected
    /// by that removal.
    #[tokio::test]
    async fn test_build_review_context_preserves_ast_and_call_graph() {
        // Arrange: a fixture PR touching a real Rust source file in this crate, with
        // deep=true to force call_graph building regardless of prompt budget (mirrors
        // the fixture setup pattern used by ast_context.rs's own build_ast_context tests).
        let repo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let mut pr = make_pr_with_content(0, 0);
        pr.files = vec![PrFile {
            filename: "src/ast_context.rs".to_string(),
            status: "modified".to_string(),
            patch: Some("+// touched".to_string()),
            patch_truncated: false,
            full_content: None,
            additions: 1,
            deletions: 0,
        }];
        let review_config = ReviewConfig::default();

        // Act
        let ctx = build_review_context(pr, Some(repo_path), true, &review_config)
            .await
            .expect("build_review_context should succeed for a valid repo_path");

        // Assert: shape is exactly what the standalone builders produce -- empty when
        // the `ast-context` feature is off, well-formed XML-tagged output when it's on.
        assert!(
            ctx.ast_context.is_empty() || ctx.ast_context.contains("<ast_context>"),
            "ast_context should be empty or well-formed XML-tagged output"
        );
        assert!(
            ctx.call_graph.is_empty() || ctx.call_graph.contains("<call_graph>"),
            "call_graph should be empty or well-formed XML-tagged output"
        );

        #[cfg(feature = "ast-context")]
        assert!(
            !ctx.ast_context.is_empty(),
            "ast_context should be populated for a real Rust fixture file when the ast-context feature is enabled"
        );
    }

    /// Regression test for issue #1596: `build_review_context` must refuse to produce
    /// a diff-less review when a non-empty PR ends up with zero surviving patches
    /// after budget drops (which would trigger a GitHub HTTP 422 downstream).
    #[tokio::test]
    async fn test_build_review_context_errs_when_all_patches_evicted() {
        // Arrange: a single file with a sizable patch and full_content, and a budget
        // so tiny that every section (including the patch, as a last resort) is dropped.
        let pr = make_pr_with_content(2000, 2000);
        let review_config = ReviewConfig {
            max_prompt_chars: 1,
            ..ReviewConfig::default()
        };

        // Act: repo_path points nowhere so ast_context/call_graph building is a no-op.
        let result = build_review_context(
            pr,
            Some("/nonexistent-aptu-test-repo-path".to_string()),
            false,
            &review_config,
        )
        .await;

        // Assert
        match result {
            Err(crate::AptuError::EmptyReviewContext { files_total }) => {
                assert_eq!(
                    files_total, 1,
                    "files_total should reflect the single PR file"
                );
            }
            other => panic!("expected EmptyReviewContext error, got {other:?}"),
        }
    }

    /// Regression test for issue #1596: an empty PR (no files) must not trigger the
    /// zero-patch guard, since `files_with_patch == 0` is vacuously true there.
    #[tokio::test]
    async fn test_build_review_context_no_err_when_files_empty() {
        // Arrange: a PR with zero files.
        let mut pr = make_pr_with_content(0, 0);
        pr.files = vec![];
        let review_config = ReviewConfig {
            max_prompt_chars: 1,
            ..ReviewConfig::default()
        };

        // Act
        let result = build_review_context(
            pr,
            Some("/nonexistent-aptu-test-repo-path".to_string()),
            false,
            &review_config,
        )
        .await;

        // Assert
        assert!(
            result.is_ok(),
            "an empty-file PR should not trigger the zero-patch guard: {result:?}"
        );
    }

    /// Regression test for issue #1596: a non-empty PR where no file ever had a
    /// patch (e.g. binary-only content) must not trigger the zero-patch guard,
    /// since `had_patches_pre_drop` is false rather than an eviction outcome.
    #[tokio::test]
    async fn test_build_review_context_no_err_when_no_patches_pre_drop() {
        // Arrange: a single file with no patch and no full_content from the start,
        // simulating a binary-only file with no diffable content.
        let mut pr = make_pr_with_content(0, 0);
        pr.files[0].patch = None;
        let review_config = ReviewConfig::default();

        // Act: repo_path points nowhere so ast_context/call_graph building is a no-op.
        let result = build_review_context(
            pr,
            Some("/nonexistent-aptu-test-repo-path".to_string()),
            false,
            &review_config,
        )
        .await;

        // Assert
        assert!(
            result.is_ok(),
            "a non-empty PR with no pre-drop patches (binary-only) should not trigger the zero-patch guard: {result:?}"
        );
    }

    /// Creates a temp directory containing multiple fixture files, for symbol-expansion
    /// tests that need `analyze_focused` to walk a small multi-file call graph.
    #[cfg(feature = "ast-context")]
    fn make_symbol_expansion_fixture(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        for (name, content) in files {
            std::fs::write(dir.path().join(name), content).expect("failed to write fixture file");
        }
        dir
    }

    /// Verifies that a changed symbol with exactly one out-of-diff caller produces a
    /// `SymbolExpansion` with correct provenance (file path and line range).
    #[cfg(feature = "ast-context")]
    #[tokio::test]
    async fn test_build_ctx_symbol_expansions_single_out_of_diff_caller() {
        let dir = make_symbol_expansion_fixture(&[
            ("changed.rs", "pub fn target_fn() {}\n"),
            ("caller_a.rs", "fn call_it() {\n    target_fn();\n}\n"),
        ]);
        let repo_path = dir.path().to_string_lossy().into_owned();
        let files = vec![PrFile {
            filename: "changed.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 0,
            patch: None,
            patch_truncated: false,
            full_content: None,
        }];

        let expansions = build_ctx_symbol_expansions(Some(&repo_path), &files, true, 5_000).await;

        assert_eq!(
            expansions.len(),
            1,
            "expected exactly one expansion for an unambiguous out-of-diff caller"
        );
        assert_eq!(expansions[0].symbol, "target_fn");
        assert_eq!(expansions[0].reference_path, "caller_a.rs");
        assert_eq!(expansions[0].reference_lines, (2, 3));
        assert!(expansions[0].snippet.contains("target_fn();"));
    }

    /// Verifies that a changed symbol with more than one out-of-diff caller is
    /// skipped entirely (ambiguous), not expanded.
    #[cfg(feature = "ast-context")]
    #[tokio::test]
    async fn test_build_ctx_symbol_expansions_ambiguous_callers_skipped() {
        let dir = make_symbol_expansion_fixture(&[
            ("changed.rs", "pub fn target_fn() {}\n"),
            ("caller_a.rs", "fn call_it() {\n    target_fn();\n}\n"),
            ("caller_b.rs", "fn call_it_too() {\n    target_fn();\n}\n"),
        ]);
        let repo_path = dir.path().to_string_lossy().into_owned();
        let files = vec![PrFile {
            filename: "changed.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 0,
            patch: None,
            patch_truncated: false,
            full_content: None,
        }];

        let expansions = build_ctx_symbol_expansions(Some(&repo_path), &files, true, 5_000).await;

        assert!(
            expansions.is_empty(),
            "ambiguous (multiple out-of-diff) callers must be skipped, got {expansions:?}"
        );
    }

    /// Creates a temp directory containing multiple fixture files nested under a
    /// subdirectory tree, for symbol-expansion tests that need `analyze_focused` to
    /// walk beyond a flat top-level layout (mimicking `crates/<crate>/src/...`).
    #[cfg(feature = "ast-context")]
    fn make_nested_symbol_expansion_fixture(
        sub_dir: &str,
        files: &[(&str, &str)],
    ) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let nested = dir.path().join(sub_dir);
        std::fs::create_dir_all(&nested).expect("failed to create nested fixture dir");
        for (name, content) in files {
            std::fs::write(nested.join(name), content).expect("failed to write fixture file");
        }
        dir
    }

    /// Regression test for the directory-walk depth bug fixed in 8366dc5: `analyze_focused`'s
    /// `max_depth` parameter is a directory-walk depth limit (`ignore::WalkBuilder::max_depth`),
    /// not a call-graph traversal depth. Passing `Some(2)`/`Some(3)` silently truncated the walk
    /// before reaching files nested under a realistic `crates/<crate>/src/` layout, so both the
    /// symbol-expansion pass and the pre-existing call-graph pass found zero callers on any
    /// normally-nested Rust repo. All other symbol-expansion fixtures above use flat files, so
    /// this test is the only one that would catch a future regression (e.g. someone re-adding a
    /// depth cap to the `analyze_focused` call in `build_symbol_expansions_context_sync`).
    #[cfg(feature = "ast-context")]
    #[tokio::test]
    async fn test_build_ctx_symbol_expansions_finds_caller_in_nested_directory() {
        let dir = make_nested_symbol_expansion_fixture(
            "crates/foo/src",
            &[
                ("changed.rs", "pub fn target_fn() {}\n"),
                ("caller_a.rs", "fn call_it() {\n    target_fn();\n}\n"),
            ],
        );
        let repo_path = dir.path().to_string_lossy().into_owned();
        let files = vec![PrFile {
            filename: "crates/foo/src/changed.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 0,
            patch: None,
            patch_truncated: false,
            full_content: None,
        }];

        let expansions = build_ctx_symbol_expansions(Some(&repo_path), &files, true, 5_000).await;

        assert_eq!(
            expansions.len(),
            1,
            "expected exactly one expansion for an out-of-diff caller nested under crates/foo/src, got {expansions:?}"
        );
        assert_eq!(expansions[0].symbol, "target_fn");
        assert_eq!(expansions[0].reference_path, "crates/foo/src/caller_a.rs");
        assert_eq!(expansions[0].reference_lines, (2, 3));
        assert!(expansions[0].snippet.contains("target_fn();"));
    }

    /// Verifies that `apply_budget_drops` clears `symbol_expansions` and records the
    /// drop when the prompt is still over budget after `ast_context` is cleared,
    /// consistent with the documented drop-order position.
    #[test]
    fn test_apply_budget_drops_symbol_expansions_dropped_over_budget() {
        let mut pr = make_pr_with_content(50, 0);
        let mut ast_context = String::new();
        let mut call_graph = String::new();
        let mut symbol_expansions = vec![SymbolExpansion {
            symbol: "foo".to_string(),
            reference_path: "src/caller.rs".to_string(),
            reference_lines: (10, 20),
            snippet: "x".repeat(400),
        }];

        // Base without symbol_expansions: patch(50) + metadata(~18) + overhead(1000) = ~1080.
        // With symbol_expansions: + snippet(400) + path(13) = ~1493.
        // Budget sits between the two, forcing the symbol_expansions drop alone.
        let max_prompt_chars = 1200;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut symbol_expansions,
            false,
            max_prompt_chars,
            &mut drops,
            None,
        );

        assert!(
            symbol_expansions.is_empty(),
            "symbol_expansions should be dropped when still over budget"
        );
        assert!(
            drops.contains(&"symbol_expansions".to_string()),
            "budget_drops should record the symbol_expansions eviction: {drops:?}"
        );
    }

    /// Verifies that `estimate_pr_size` includes `symbol_expansions`' `snippet` and
    /// `reference_path` char lengths in the total, so the budget cap actually applies.
    #[test]
    fn test_estimate_pr_size_includes_symbol_expansions() {
        let pr = make_pr_with_content(0, 0);
        let symbol_expansions = vec![SymbolExpansion {
            symbol: "foo".to_string(),
            reference_path: "src/caller.rs".to_string(),
            reference_lines: (1, 5),
            snippet: "fn foo() {}".to_string(),
        }];
        let size = estimate_pr_size(&pr, "", "", &symbol_expansions);
        let without_expansions = estimate_pr_size(&pr, "", "", &[]);
        let expected_delta =
            symbol_expansions[0].snippet.len() + symbol_expansions[0].reference_path.len();
        assert_eq!(size - without_expansions, expected_delta);
    }
}
