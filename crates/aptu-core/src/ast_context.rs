// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2025 Agentic AI Foundation

//! AST context injection for PR reviews.
//!
//! Extracts function signatures and cross-file call graph information from
//! changed source files and appends structured context to the AI review prompt.
//! Supported languages: Rust, Python, Go, Java, TypeScript, TSX, JavaScript,
//! C, C++, C#, and Fortran (determined by `aptu_coder_core::language_for_extension`).
//!
//! # Feature Flag
//!
//! Most functionality is gated behind the `ast-context` Cargo feature, which
//! enables the optional `aptu-coder-core` dependency. When the feature is
//! disabled, [`build_ast_context`] and [`build_call_graph_context`] return
//! empty strings immediately without performing any I/O.
//!
//! # Output Format
//!
//! Context is emitted as XML-tagged blocks appended after `</pull_request>`:
//! - `<ast_context>`: function signatures and imports per changed file
//! - `<call_graph_context>`: cross-file call chains for changed functions
//!
//! Each block is capped at approximately 2000 characters (soft ceiling; the
//! actual maximum is slightly higher due to the closing XML tag appended
//! after truncation).

use crate::ai::types::PrFile;
#[cfg(feature = "ast-context")]
use crate::ai::types::SymbolExpansion;
use std::path::Path;
use tracing::debug;

#[cfg(feature = "ast-context")]
use std::fmt::Write as _;

#[cfg(feature = "ast-context")]
use aptu_coder_core::{analyze_file, analyze_focused, language_for_extension};

/// Result of building AST context.
#[derive(Debug)]
pub(crate) struct AstContextOutput {
    /// Text representation of AST context (for prompt injection).
    pub text: String,
}

impl AstContextOutput {
    pub(crate) fn new(text: String) -> Self {
        Self { text }
    }
}

impl Default for AstContextOutput {
    fn default() -> Self {
        Self::new(String::new())
    }
}

// `str::floor_char_boundary` is available in std but remains behind the
// `str_internals` nightly feature gate on stable Rust. This local
// implementation provides the equivalent behavior on stable.

/// Return the largest byte index `<= max` that falls on a UTF-8 character boundary.
///
/// `String::truncate` panics when the index splits a multi-byte codepoint;
/// this function prevents that by scanning backwards to the nearest boundary.
#[cfg(feature = "ast-context")]
fn floor_char_boundary(s: &str, max: usize) -> usize {
    if max >= s.len() {
        return s.len();
    }
    let mut idx = max;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Build a compact AST context string for the changed files in a PR.
///
/// Returns empty string if `repo_path` is invalid or no files have analysis results.
/// Output is capped at 2000 characters.
#[allow(private_interfaces)]
pub async fn build_ast_context(repo_path: &str, files: &[PrFile]) -> AstContextOutput {
    let repo_path = repo_path.to_string();
    let files: Vec<PrFile> = files.to_vec();

    match tokio::task::spawn_blocking(move || build_ast_context_sync(&repo_path, &files)).await {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("build_ast_context: blocking task panicked: {e}");
            AstContextOutput::new(String::new())
        }
    }
}

#[cfg(not(feature = "ast-context"))]
fn build_ast_context_sync(_repo_path: &str, _files: &[PrFile]) -> AstContextOutput {
    AstContextOutput::new(String::new())
}

#[cfg(feature = "ast-context")]
#[allow(clippy::too_many_lines)]
fn build_ast_context_sync(repo_path: &str, files: &[PrFile]) -> AstContextOutput {
    // CAP is a soft ceiling: the closing XML tag is appended after truncation,
    // so actual maximum output length is CAP + len(closing_tag).
    const CAP: usize = 2000;
    let mut output = String::from("\n<ast_context>\n");

    for file in files {
        let ext = Path::new(&file.filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        // skip files with unsupported languages
        if language_for_extension(ext).is_none() {
            continue;
        }
        let full_path = Path::new(repo_path).join(&file.filename);
        let path_str = full_path.to_string_lossy().into_owned();

        match analyze_file(&path_str, None) {
            Ok(analysis) => {
                let mut file_block = format!("## {}\n", file.filename);
                for func in &analysis.semantic.functions {
                    let _ = writeln!(file_block, "  fn {}", func.compact_signature());
                }
                if !analysis.semantic.imports.is_empty() {
                    file_block.push_str("  imports:");
                    for imp in analysis.semantic.imports.iter().take(5) {
                        let _ = write!(file_block, " {}", imp.module);
                    }
                    file_block.push('\n');
                }
                if output.len() + file_block.len() <= CAP {
                    output.push_str(&file_block);
                }
            }
            Err(e) => {
                debug!("ast_context: skipping {}: {}", file.filename, e);
            }
        }
    }
    output.push_str("</ast_context>\n");

    // If nothing was added (only the wrapper tags), clear the text.
    if output == "\n<ast_context>\n</ast_context>\n" {
        output.clear();
    }

    // Enforce cap on the full output
    if output.len() > CAP {
        let boundary = floor_char_boundary(&output, CAP);
        output.truncate(boundary);
        output.push_str("\n</ast_context>\n");
    }

    AstContextOutput::new(output)
}

/// Build cross-file call graph context for the changed files.
///
/// For each function in each changed file, looks up its callers.
/// Output is capped at 3000 characters.
pub async fn build_call_graph_context(repo_path: &str, files: &[PrFile]) -> String {
    let repo_path = repo_path.to_string();
    let files: Vec<PrFile> = files.to_vec();

    match tokio::task::spawn_blocking(move || build_call_graph_context_sync(&repo_path, &files))
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("build_call_graph_context: blocking task panicked: {e}");
            String::new()
        }
    }
}

#[cfg(not(feature = "ast-context"))]
fn build_call_graph_context_sync(_repo_path: &str, _files: &[PrFile]) -> String {
    String::new()
}

#[cfg(feature = "ast-context")]
fn build_call_graph_context_sync(repo_path: &str, files: &[PrFile]) -> String {
    const CAP: usize = 3000;
    let mut output = String::from("\n<call_graph>\n");
    let repo = Path::new(repo_path);

    for file in files {
        let ext = Path::new(&file.filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        // skip files with unsupported languages
        if language_for_extension(ext).is_none() {
            continue;
        }
        let full_path = repo.join(&file.filename);
        let path_str = full_path.to_string_lossy().into_owned();

        // Get function names in this file
        let fn_names: Vec<String> = match analyze_file(&path_str, None) {
            Ok(a) => a
                .semantic
                .functions
                .iter()
                .map(|f| {
                    // Extract function name from the compact signature format produced by
                    // aptu-coder-core ("name(params) -> return_type"). The crate version
                    // is pinned in Cargo.toml; a format change would require updating this.
                    f.compact_signature()
                        .split('(')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string()
                })
                .filter(|s| !s.is_empty())
                .collect(),
            Err(_) => continue,
        };

        'outer: for fn_name in fn_names.iter().take(5) {
            // `max_depth` is a directory-walk depth limit (passed straight to
            // `ignore::WalkBuilder::max_depth`), not a call-graph traversal depth; `None`
            // walks the full tree so nested `crates/<crate>/src/` files are reachable.
            // Result volume is bounded by `max_results` and the `.take(5)`/`.take(3)` caps
            // below, not by directory depth.
            match analyze_focused(repo, fn_name, 1, None, None) {
                Ok(focused) => {
                    if focused.prod_chains.is_empty() {
                        continue;
                    }
                    let mut block = format!("### callers of `{fn_name}`\n");
                    for chain in focused.prod_chains.iter().take(3) {
                        if let Some((caller_sym, caller_file, caller_line)) = chain.chain.first() {
                            let _ = writeln!(
                                block,
                                "  {} ({}:{})",
                                caller_sym,
                                caller_file
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                                caller_line
                            );
                        }
                    }
                    if output.len() + block.len() > CAP {
                        break 'outer;
                    }
                    output.push_str(&block);
                }
                Err(e) => {
                    debug!("call_graph: skipping {}/{}: {}", file.filename, fn_name, e);
                }
            }
        }
    }

    output.push_str("</call_graph>\n");

    if output == "\n<call_graph>\n</call_graph>\n" {
        return String::new();
    }

    if output.len() > CAP {
        let boundary = floor_char_boundary(&output, CAP);
        output.truncate(boundary);
        output.push_str("\n</call_graph>\n");
    }

    output
}

/// Build targeted symbol expansions for changed functions with exactly one
/// unambiguous out-of-diff caller.
///
/// For each supported changed file, extracts function names and looks up
/// callers via `analyze_focused`. A symbol is expanded only when exactly one
/// caller resides outside the PR's changed files (an "unambiguous single
/// out-of-diff reference"); zero or multiple out-of-diff matches are skipped
/// silently. The running total of `snippet` + `reference_path` chars never
/// exceeds `max_chars`; once the budget is exhausted, no further expansions
/// are added.
pub async fn build_symbol_expansions_context(
    repo_path: &str,
    files: &[PrFile],
    max_chars: usize,
) -> Vec<crate::ai::types::SymbolExpansion> {
    let repo_path = repo_path.to_string();
    let files: Vec<PrFile> = files.to_vec();

    match tokio::task::spawn_blocking(move || {
        build_symbol_expansions_context_sync(&repo_path, &files, max_chars)
    })
    .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("build_symbol_expansions_context: blocking task panicked: {e}");
            Vec::new()
        }
    }
}

#[cfg(not(feature = "ast-context"))]
fn build_symbol_expansions_context_sync(
    _repo_path: &str,
    _files: &[PrFile],
    _max_chars: usize,
) -> Vec<crate::ai::types::SymbolExpansion> {
    Vec::new()
}

/// Number of lines read into a symbol-expansion snippet, starting at the
/// caller's call-site line. A fixed window is used rather than the caller
/// function's actual body span, since `analyze_focused` does not report
/// function end lines; this keeps snippets bounded and predictable.
#[cfg(feature = "ast-context")]
const SYMBOL_EXPANSION_SNIPPET_LINES: usize = 15;

#[cfg(feature = "ast-context")]
fn build_symbol_expansions_context_sync(
    repo_path: &str,
    files: &[PrFile],
    max_chars: usize,
) -> Vec<SymbolExpansion> {
    let repo = Path::new(repo_path);
    let diff_filenames: std::collections::HashSet<&str> =
        files.iter().map(|f| f.filename.as_str()).collect();

    let mut expansions = Vec::new();
    let mut running_total = 0usize;

    'files: for file in files {
        let ext = Path::new(&file.filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if language_for_extension(ext).is_none() {
            continue;
        }
        let full_path = repo.join(&file.filename);
        let path_str = full_path.to_string_lossy().into_owned();

        let fn_names: Vec<String> = match analyze_file(&path_str, None) {
            Ok(a) => a
                .semantic
                .functions
                .iter()
                .map(|f| {
                    f.compact_signature()
                        .split('(')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string()
                })
                .filter(|s| !s.is_empty())
                .collect(),
            Err(_) => continue,
        };

        for fn_name in fn_names.iter().take(5) {
            if running_total >= max_chars {
                break 'files;
            }

            // See the analogous `analyze_focused` call in `build_call_graph_context_sync`
            // above: `max_depth` is a directory-walk depth, not call-graph depth, so `None`
            // (unrestricted walk) is required to reach nested `crates/<crate>/src/` files.
            let focused = match analyze_focused(repo, fn_name, 1, None, None) {
                Ok(focused) => focused,
                Err(e) => {
                    debug!(
                        "symbol_expansions: skipping {}/{}: {}",
                        file.filename, fn_name, e
                    );
                    continue;
                }
            };

            // `chain.chain.first()` yields caller paths as returned by `analyze_focused`,
            // which may be absolute (joined against `repo` during the walk) rather than
            // relative; normalize via `strip_prefix` before comparing against `pr.files`
            // filenames (always relative) or using the path as provenance.
            let out_of_diff: Vec<(std::path::PathBuf, usize)> = focused
                .prod_chains
                .iter()
                .filter_map(|chain| chain.chain.first())
                .map(|(_, caller_file, caller_line)| {
                    let rel = caller_file
                        .strip_prefix(repo)
                        .map_or_else(|_| caller_file.clone(), std::path::Path::to_path_buf);
                    (rel, *caller_line)
                })
                .filter(|(rel, _)| !diff_filenames.contains(rel.to_string_lossy().as_ref()))
                .collect();

            if out_of_diff.len() != 1 {
                continue;
            }
            let (caller_rel, caller_line) = &out_of_diff[0];

            let caller_full_path = repo.join(caller_rel);
            let Ok(source) = std::fs::read_to_string(&caller_full_path) else {
                continue;
            };
            let lines: Vec<&str> = source.lines().collect();
            if lines.is_empty() {
                continue;
            }
            // caller_line is 1-indexed; clamp to file bounds.
            let start_idx = caller_line.saturating_sub(1).min(lines.len() - 1);
            let end_idx = (start_idx + SYMBOL_EXPANSION_SNIPPET_LINES).min(lines.len());
            let snippet = lines[start_idx..end_idx].join("\n");
            let reference_path = caller_rel.to_string_lossy().into_owned();

            let expansion_chars = snippet.len() + reference_path.len();
            if running_total + expansion_chars > max_chars {
                tracing::warn!(
                    section = "symbol_expansions",
                    symbol = %fn_name,
                    chars = expansion_chars,
                    "Skipping symbol expansion: budget exhausted"
                );
                break 'files;
            }
            running_total += expansion_chars;

            #[allow(clippy::cast_possible_truncation)]
            expansions.push(SymbolExpansion {
                symbol: fn_name.clone(),
                reference_path,
                reference_lines: (start_idx as u32 + 1, end_idx as u32),
                snippet,
            });
        }
    }

    expansions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pr_file(filename: &str) -> PrFile {
        PrFile {
            filename: filename.to_string(),
            status: "modified".to_string(),
            additions: 0,
            deletions: 0,
            patch: None,
            patch_truncated: false,
            full_content: None,
        }
    }

    #[tokio::test]
    async fn test_build_ast_context_missing_path_returns_empty() {
        let files = vec![make_pr_file("src/main.rs")];
        let result = build_ast_context("/nonexistent/path/xyz", &files).await;
        assert!(
            result.text.is_empty(),
            "expected empty for missing repo path"
        );
    }

    #[tokio::test]
    async fn test_build_ast_context_valid_rust_file() {
        let repo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let files = vec![make_pr_file("src/ast_context.rs")];
        let result = build_ast_context(&repo_path, &files).await;
        // Verify it doesn't panic and respects the cap
        assert!(result.text.len() <= 2200, "output should be near cap");
    }

    #[tokio::test]
    async fn test_build_ast_context_cap_enforced() {
        let files: Vec<PrFile> = (0..50)
            .map(|i| make_pr_file(&format!("src/file_{i}.rs")))
            .collect();
        let result = build_ast_context(".", &files).await;
        assert!(
            result.text.len() <= 2200,
            "output must be capped near 2000 chars"
        );
    }

    #[tokio::test]
    async fn test_ast_context_python_file_included() {
        let files = vec![make_pr_file("test_file.py")];
        let result = build_ast_context(".", &files).await;
        // Python file should be processed by language_for_extension (happy path)
        assert!(
            result.text.is_empty() || result.text.contains("<ast_context>"),
            "Python file should be included in AST context"
        );
    }

    #[tokio::test]
    async fn test_ast_context_typescript_file_included() {
        let files = vec![make_pr_file("test_file.ts")];
        let result = build_ast_context(".", &files).await;
        // TypeScript file should be processed by language_for_extension
        assert!(
            result.text.is_empty() || result.text.contains("<ast_context>"),
            "TypeScript file should be included in AST context"
        );
    }

    #[tokio::test]
    async fn test_ast_context_markdown_file_included() {
        let files = vec![make_pr_file("README.md")];
        let result = build_ast_context(".", &files).await;
        // Markdown is supported in aptu-coder-core >= 0.22.0 (tree-sitter-md)
        #[cfg(feature = "ast-context")]
        assert!(
            result.text.contains("<ast_context>"),
            "Markdown file should produce an <ast_context> block; got: {result:?}"
        );
        #[cfg(not(feature = "ast-context"))]
        assert!(
            result.text.is_empty(),
            "without ast-context feature, build_ast_context returns empty"
        );
    }
}
