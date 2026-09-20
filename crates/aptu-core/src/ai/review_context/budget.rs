// SPDX-License-Identifier: Apache-2.0

//! Prompt size estimation and budget-driven enrichment drops.

#[cfg(feature = "ast-context")]
use std::fmt::Write as _;

#[cfg(feature = "ast-context")]
use aptu_coder_core::{analyze_file, language_for_extension};

use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

/// Estimated overhead for XML tags, section headers, and schema preamble added by
/// `build_pr_review_user_prompt`. Used to ensure the prompt budget accounts for
/// non-content characters when estimating total prompt size.
pub(crate) const PROMPT_OVERHEAD_CHARS: usize = 1_000;

/// Determines whether to enable call graph context based on the remaining
/// prompt budget. Call-graph context is auto-enabled whenever the estimated
/// prompt leaves more than `min_budget_for_call_graph` characters available.
pub(crate) fn should_enable_call_graph(budget_remaining: usize, config: &ReviewConfig) -> bool {
    budget_remaining > config.min_budget_for_call_graph
}

/// Applies budget drop order: `call_graph` -> `ast_context` ->
/// `dep_enrichments` -> `full_content` -> patches.
/// Enforces the prompt budget by dropping enrichment sections in priority order.
///
/// When the assembled prompt exceeds `max_prompt_chars`, sections are cleared in
/// the following order (lowest-priority dropped first):
///
/// 1. `call_graph` -- dropped first
/// 2. `ast_context` -- dropped second
/// 3. `dep_enrichments` -- dropped third
/// 4. file `full_content` -- dropped largest-first
/// 5. file patches -- dropped largest-first as last resort, so the diff itself
///    (the highest-value context for a review) is preserved as long as possible
///
/// Each drop is logged at `WARN` level with the section name and character count.
/// The function never returns an error; sections that cannot fit are silently cleared.
pub(crate) fn apply_budget_drops(
    pr: &mut PrDetails,
    ast_context: &mut String,
    call_graph: &mut String,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
    repo_path: Option<&str>,
) {
    let mut estimated_size = estimate_pr_size(pr, ast_context, call_graph);

    // Drop call_graph if over budget
    if estimated_size > max_prompt_chars {
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
pub(crate) fn drop_dep_enrichments_by_size(
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
pub(crate) fn drop_patches_by_size(
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
pub(crate) fn build_file_outline(repo_path: &str, filename: &str) -> Option<String> {
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
pub(crate) fn build_file_outline(_repo_path: &str, _filename: &str) -> Option<String> {
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
pub(crate) fn drop_full_content_by_size(
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
/// `ast_context`, `call_graph`, and overhead.
#[must_use]
pub(crate) fn estimate_pr_size(pr: &PrDetails, ast_context: &str, call_graph: &str) -> usize {
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

    // Overhead
    size += PROMPT_OVERHEAD_CHARS;

    size
}
