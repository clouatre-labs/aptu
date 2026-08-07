// SPDX-License-Identifier: Apache-2.0

//! Review context policy layer for PR analysis.
//!
//! Centralizes all enrichment decisions (AST context, call graph, dependency enrichments)
//! and CWD inference into a single `ReviewContext` struct and `build_review_context()` function.

use std::path::PathBuf;

use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

#[cfg(all(feature = "ast-context", feature = "graph"))]
use regex::Regex;
#[cfg(all(feature = "ast-context", feature = "graph"))]
use std::sync::LazyLock;

/// Regex to extract symbol names from unified-diff added lines.
/// Matches `fn`/`async fn`, `struct`, `enum`, `trait`, and `impl` declarations,
/// stripping any visibility prefix (including `pub(crate)`, `pub(super)`, etc.).
/// Capture group 2 is the keyword, capture group 3 is the symbol name.
#[cfg(all(feature = "ast-context", feature = "graph"))]
static SYMBOL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\+(\s*)(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(fn|struct|enum|trait|impl)\s+([a-zA-Z_]\w*)",
    )
    .expect("valid SYMBOL_RE")
});

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
    /// Structural graph subgraph text for prompt injection (empty when graph feature is disabled).
    pub graph_context: String,
    /// Whether the structural graph was loaded from the on-disk cache (false when feature is off).
    pub graph_cache_hit: bool,
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
            graph_context: String::new(),
            graph_cache_hit: false,
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
    graph_config: &crate::config::GraphConfig,
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
    // (which carries both the text string and the structural GraphDb).
    // When disabled, it returns a plain String.
    #[cfg(feature = "ast-context")]
    let ast_output = build_ctx_ast(repo_path_ref.as_deref(), &pr.files).await;
    #[cfg(feature = "ast-context")]
    let ast_context = ast_output.text.clone();
    #[cfg(not(feature = "ast-context"))]
    let ast_output = build_ctx_ast(repo_path_ref.as_deref(), &pr.files).await;
    #[cfg(not(feature = "ast-context"))]
    let ast_context = ast_output.clone();

    // Step 3: Enrich with dependency release notes
    pr.dep_enrichments = enrich_deps(&pr.files, review_config).await;

    // Step 4: Estimate total chars and decide call_graph budget
    // (call_graph and graph_context not yet built, pass empty strings)
    let estimated_size = estimate_pr_size(&pr, &ast_context, "", "");
    let max_prompt_chars = review_config.max_prompt_chars;
    let budget_remaining = max_prompt_chars.saturating_sub(estimated_size);

    // Step 5: Build call_graph if decided
    let should_enable_cg = should_enable_call_graph(deep, budget_remaining, review_config);
    let mut call_graph = if should_enable_cg {
        build_ctx_call_graph(repo_path_ref.as_deref(), &pr.files, true).await
    } else {
        String::new()
    };

    // Re-estimate with actual call_graph for accurate routing (graph_context still empty here)
    let final_estimated_size = estimate_pr_size(&pr, &ast_context, &call_graph, "");

    // Step 5b: Build structural graph context if enabled
    let (mut graph_context, graph_cache_hit) =
        build_ctx_graph(graph_config, repo_path_ref.as_deref(), &pr, &ast_output).await;

    // Step 6: Apply budget drop order
    let mut ast_context = ast_context;
    let mut budget_drops = Vec::new();
    apply_budget_drops(
        &mut pr,
        &mut ast_context,
        &mut call_graph,
        &mut graph_context,
        deep,
        max_prompt_chars,
        &mut budget_drops,
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
        graph_context,
        graph_cache_hit,
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

/// Applies budget drop order: `call_graph` -> `graph_context` -> `ast_context` -> `dep_enrichments` -> patches -> `full_content`.
/// Enforces the prompt budget by dropping enrichment sections in priority order.
///
/// When the assembled prompt exceeds `max_prompt_chars`, sections are cleared in
/// the following order (lowest-priority dropped first):
///
/// 1. `call_graph` -- dropped first unless `deep` is explicitly set
/// 2. `graph_context` -- dropped second (petgraph blast-radius subgraph; added in #1408)
/// 3. `ast_context` -- dropped third
/// 4. `dep_enrichments` -- dropped fourth
/// 5. file patches -- dropped largest-first
/// 6. file `full_content` -- dropped largest-first as last resort
///
/// Each drop is logged at `WARN` level with the section name and character count.
/// The function never returns an error; sections that cannot fit are silently cleared.
fn apply_budget_drops(
    pr: &mut PrDetails,
    ast_context: &mut String,
    call_graph: &mut String,
    graph_context: &mut String,
    deep: bool,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
) {
    let mut estimated_size = estimate_pr_size(pr, ast_context, call_graph, graph_context);

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

    // Drop graph_context second (priority tier 2: between call_graph and ast_context; added in #1408).
    if estimated_size > max_prompt_chars {
        tracing::warn!(
            section = "graph_context",
            priority_tier = 2,
            chars = graph_context.len(),
            "Dropping section: prompt budget exceeded (graph_context tier)"
        );
        let dropped_chars = graph_context.len();
        graph_context.clear();
        estimated_size -= dropped_chars;
        budget_drops.push("graph_context".to_string());
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

    drop_dep_enrichments_by_size(pr, &mut estimated_size, max_prompt_chars, budget_drops);

    drop_patches_by_size(
        &mut pr.files,
        &mut estimated_size,
        max_prompt_chars,
        budget_drops,
    );
    drop_full_content_by_size(
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
///
/// Sums title, body, file metadata, patches, `full_content`, `dep_enrichments`,
/// `ast_context`, `call_graph`, and overhead.
#[must_use]
pub(crate) fn estimate_pr_size(
    pr: &PrDetails,
    ast_context: &str,
    call_graph: &str,
    graph_context: &str,
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

    // Structural graph context
    size += graph_context.len();

    // Overhead
    size += PROMPT_OVERHEAD_CHARS;

    size
}

/// Builds AST context for changed files.
///
/// Returns an [`AstContextOutput`] containing the text string and (when the `graph`
/// feature is also enabled) the structural [`GraphDb`] built from the same analysis.
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

/// Builds structural graph context from PR-changed files when both `ast-context` and
/// `graph` features are enabled.
///
/// Accepts a pre-built [`AstContextOutput`] containing the structural graph from
/// `ast_context.rs`, and passes it to `cache::load_or_build` for caching.
///
/// Returns `(rendered_text, cache_hit)`. Returns empty string and `false` when the
/// feature is off, when `graph_config.enabled` is false, or when `repo_path` is absent.
#[allow(clippy::unused_async)]
#[cfg(feature = "ast-context")]
async fn build_ctx_graph(
    graph_config: &crate::config::GraphConfig,
    repo_path: Option<&str>,
    pr: &PrDetails,
    ast_output: &crate::ast_context::AstContextOutput,
) -> (String, bool) {
    #[cfg(feature = "graph")]
    {
        if !graph_config.enabled {
            return (String::new(), false);
        }
        let Some(_repo_path_str) = repo_path else {
            return (String::new(), false);
        };
        let sha = pr.head_sha.clone();
        let owner_str = pr.owner.clone();
        let repo_str = pr.repo.clone();
        let graph_config_owned = graph_config.clone();
        let graph_owned = ast_output.graph.clone();
        let function_names: Vec<String> = derive_modified_symbols(&pr.files);

        let spawn_result = tokio::task::spawn_blocking(move || {
            let (mut graph, cache_hit) = crate::graph::cache::load_or_build(
                &owner_str,
                &repo_str,
                &sha,
                graph_owned,
                &graph_config_owned,
            );
            let fn_refs: Vec<&str> = function_names.iter().map(String::as_str).collect();
            let modified_nodes = crate::graph::query::find_modified_nodes(&mut graph, &fn_refs);
            let subgraph = crate::graph::query::blast_radius(
                &graph,
                &modified_nodes,
                graph_config_owned.max_nodes,
                graph_config_owned.max_depth,
            );
            (
                crate::graph::query::render_subgraph_text(&subgraph),
                cache_hit,
            )
        })
        .await;

        match spawn_result {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("graph cache spawn_blocking panicked: {e}");
                (String::new(), false)
            }
        }
    }
    #[cfg(not(feature = "graph"))]
    {
        let _ = (graph_config, repo_path, pr, ast_output);
        (String::new(), false)
    }
}

/// Stub when `ast-context` feature is off: graph context always empty.
#[allow(clippy::unused_async)]
#[cfg(not(feature = "ast-context"))]
async fn build_ctx_graph(
    graph_config: &crate::config::GraphConfig,
    repo_path: Option<&str>,
    pr: &PrDetails,
    _ast_text: &str,
) -> (String, bool) {
    let _ = (graph_config, repo_path, pr);
    (String::new(), false)
}

/// Derives modified symbol names from PR file patches by parsing diff hunks.
///
/// Iterates `pr.files`, parses each patch string for `@@` hunk headers to get
/// changed line ranges, then matches added/changed lines against
/// `fn`/`struct`/`enum`/`trait`/`impl` name patterns to extract symbol names.
///
/// Handles:
/// - `patch = None` -> empty `Vec`
/// - `patch_truncated = true` -> partial set (uses what's available)
/// - Renamed files (status field) -> treated same as modified for extraction
#[cfg(all(feature = "ast-context", feature = "graph"))]
fn derive_modified_symbols(files: &[crate::ai::types::PrFile]) -> Vec<String> {
    let mut symbols: Vec<String> = Vec::new();

    for file in files {
        let Some(patch) = &file.patch else {
            continue;
        };

        // Parse each line of the patch for symbol definitions
        for line in patch.lines() {
            // Skip hunk headers, removed lines, and file-header lines
            if line.starts_with("@@") || line.starts_with('-') || line.starts_with("+++") {
                continue;
            }

            // Match added lines with symbol declarations using regex
            if let Some(caps) = SYMBOL_RE.captures(line) {
                let keyword = caps.get(2).map_or("", |m| m.as_str());
                let name = caps.get(3).map_or("", |m| m.as_str()).to_string();

                let sym = if keyword == "impl" {
                    // For "impl Trait for Type", extract the type name after "for"
                    let trimmed = line.strip_prefix('+').unwrap_or("").trim();
                    let after_impl = trimmed.strip_prefix("impl ").unwrap_or("");
                    if let Some(for_pos) = after_impl.find(" for ") {
                        after_impl[for_pos + 5..]
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_string()
                    } else {
                        name
                    }
                } else {
                    name
                };

                if !sym.is_empty() && !symbols.contains(&sym) {
                    symbols.push(sym);
                }
            }
        }
    }

    symbols
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

    /// Verifies that apply_budget_drops enforces the documented drop order:
    /// call_graph -> ast_context -> dep_enrichments -> patches -> full_content.
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
        let mut graph_context = String::new();
        // Updated in #1408: apply_budget_drops now takes graph_context as a new drop tier
        // between call_graph and ast_context. Priority order:
        // call_graph -> graph_context -> ast_context -> dep_enrichments -> patches -> full_content
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut graph_context,
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
        let mut graph_context = String::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            &mut graph_context,
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
        let size = estimate_pr_size(&pr, ast_context, call_graph, "");
        let without_call_graph = estimate_pr_size(&pr, ast_context, "", "");
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
        let size = estimate_pr_size(&pr, ast_context, call_graph, "");
        assert!(
            size >= call_graph.len() + PROMPT_OVERHEAD_CHARS,
            "estimated size {} should be >= call_graph.len() {} + overhead {}",
            size,
            call_graph.len(),
            PROMPT_OVERHEAD_CHARS
        );
    }

    // -----------------------------------------------------------------------
    // derive_modified_symbols tests
    // -----------------------------------------------------------------------

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[test]
    fn test_modified_symbols_patch_none_yields_empty() {
        // Arrange: file with no patch
        let files = vec![PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: None,
            patch_truncated: false,
            full_content: None,
            additions: 0,
            deletions: 0,
        }];

        // Act
        let symbols = derive_modified_symbols(&files);

        // Assert
        assert!(symbols.is_empty(), "patch=None should yield empty symbols");
    }

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[test]
    fn test_modified_symbols_extracts_from_hunk_lines() {
        // Arrange: patch with fn, struct, and enum definitions in added lines
        let patch = "\
@@ -1,5 +1,10 @@
 fn existing_fn() {}
+fn new_fn() -> Result<()> {
+    Ok(())
+}
+pub struct NewStruct {
+    field: i32,
+}
+pub enum NewEnum {
+    VariantA,
+}
+impl NewStruct {
+    fn method(&self) {}
+}
";
        let files = vec![PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: Some(patch.to_string()),
            patch_truncated: false,
            full_content: None,
            additions: 5,
            deletions: 0,
        }];

        // Act
        let mut symbols = derive_modified_symbols(&files);
        symbols.sort();

        // Assert
        assert!(
            symbols.contains(&"new_fn".to_string()),
            "should extract fn name"
        );
        assert!(
            symbols.contains(&"NewStruct".to_string()),
            "should extract struct name"
        );
        assert!(
            symbols.contains(&"NewEnum".to_string()),
            "should extract enum name"
        );
        // 'impl' block: the type after 'impl' is extracted
        assert!(
            symbols.contains(&"NewStruct".to_string()),
            "impl target should be extracted"
        );
    }

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[test]
    fn test_modified_symbols_patch_truncated_yields_partial() {
        // Arrange: truncated patch (patch_truncated=true) with some definitions
        let patch = "\
@@ -1,5 +1,8 @@
 fn existing_fn() {}
+fn visible_fn() {}
+pub struct VisibleStruct {
+    field: i32,
+}
";
        let files = vec![PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: Some(patch.to_string()),
            patch_truncated: true,
            full_content: None,
            additions: 3,
            deletions: 0,
        }];

        // Act
        let symbols = derive_modified_symbols(&files);

        // Assert: partial set from available patch content
        assert!(
            symbols.contains(&"visible_fn".to_string()),
            "should extract fn from truncated patch"
        );
        assert!(
            symbols.contains(&"VisibleStruct".to_string()),
            "should extract struct from truncated patch"
        );
    }

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[test]
    fn test_modified_symbols_renamed_file_treated_as_modified() {
        // Arrange: renamed file with a function definition
        let patch = "\
@@ -1,1 +1,1 @@
-fn old_name() {}
+fn renamed_fn() {}
";
        let files = vec![PrFile {
            filename: "src/renamed.rs".to_string(),
            status: "renamed".to_string(),
            patch: Some(patch.to_string()),
            patch_truncated: false,
            full_content: None,
            additions: 1,
            deletions: 1,
        }];

        // Act
        let symbols = derive_modified_symbols(&files);

        // Assert: renamed files are treated same as modified
        assert!(
            symbols.contains(&"renamed_fn".to_string()),
            "should extract fn from renamed file"
        );
    }

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[test]
    fn test_modified_symbols_async_fn() {
        // Arrange: patch with async fn declarations
        let patch = "\
@@ -1,3 +1,6 @@
 fn sync_fn() {}
+async fn fetch_data() -> Result<()> {
+    Ok(())
+}
+pub async fn handle_request() -> String {
+    String::new()
+}
";
        let files = vec![PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: Some(patch.to_string()),
            patch_truncated: false,
            full_content: None,
            additions: 2,
            deletions: 0,
        }];

        // Act
        let mut symbols = derive_modified_symbols(&files);
        symbols.sort();

        // Assert
        assert!(
            symbols.contains(&"fetch_data".to_string()),
            "should extract async fn name"
        );
        assert!(
            symbols.contains(&"handle_request".to_string()),
            "should extract pub async fn name"
        );
    }

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[test]
    fn test_modified_symbols_pub_visibility() {
        // Arrange: patch with pub(crate) and pub(super) visibility
        let patch = "\
@@ -1,3 +1,5 @@
 fn existing() {}
+pub(crate) fn internal_fn() -> i32 { 42 }
+pub(super) fn super_fn() -> bool { true }
";
        let files = vec![PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: Some(patch.to_string()),
            patch_truncated: false,
            full_content: None,
            additions: 2,
            deletions: 0,
        }];

        // Act
        let mut symbols = derive_modified_symbols(&files);
        symbols.sort();

        // Assert
        assert!(
            symbols.contains(&"internal_fn".to_string()),
            "should extract fn with pub(crate) visibility"
        );
        assert!(
            symbols.contains(&"super_fn".to_string()),
            "should extract fn with pub(super) visibility"
        );
    }

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[test]
    fn test_modified_symbols_generic_fn() {
        // Arrange: patch with generic function signatures
        let patch = "\
@@ -1,3 +1,5 @@
 fn existing() {}
+fn generic_fn<T: Debug>(x: T) -> String { format!(\"{:?}\", x) }
+fn multi_bound_fn<T: Clone + Debug, U: Display>(a: T, b: U) {}
";
        let files = vec![PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: Some(patch.to_string()),
            patch_truncated: false,
            full_content: None,
            additions: 2,
            deletions: 0,
        }];

        // Act
        let mut symbols = derive_modified_symbols(&files);
        symbols.sort();

        // Assert
        assert!(
            symbols.contains(&"generic_fn".to_string()),
            "should extract generic fn name without generic params"
        );
        assert!(
            symbols.contains(&"multi_bound_fn".to_string()),
            "should extract multi-bound generic fn name"
        );
    }

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[test]
    fn test_modified_symbols_tuple_unit_structs() {
        // Arrange: patch with tuple struct and unit struct definitions
        let patch = "\
@@ -1,3 +1,6 @@
 fn existing() {}
+struct Point(i32, i32);
+struct Unit;
+pub struct Named {
+    field: i32,
+}
";
        let files = vec![PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: Some(patch.to_string()),
            patch_truncated: false,
            full_content: None,
            additions: 3,
            deletions: 0,
        }];

        // Act
        let mut symbols = derive_modified_symbols(&files);
        symbols.sort();

        // Assert
        assert!(
            symbols.contains(&"Point".to_string()),
            "should extract tuple struct name"
        );
        assert!(
            symbols.contains(&"Unit".to_string()),
            "should extract unit struct name"
        );
        assert!(
            symbols.contains(&"Named".to_string()),
            "should extract named struct name"
        );
    }
}
