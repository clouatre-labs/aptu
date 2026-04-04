// SPDX-License-Identifier: Apache-2.0

//! AST context injection for PR reviews.
//!
//! Extracts function signatures and cross-file call graph information from
//! changed files and appends them to the AI review prompt.

use crate::ai::types::PrFile;
use std::path::Path;
use tracing::debug;

#[cfg(feature = "ast-context")]
use std::fmt::Write as _;

#[cfg(feature = "ast-context")]
use code_analyze_core::{analyze_file, analyze_focused};

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

#[cfg(feature = "ast-context")]
fn is_rust_file(filename: &str) -> bool {
    Path::new(filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

/// Build a compact AST context string for the changed files in a PR.
///
/// Returns empty string if `repo_path` is invalid or no files have analysis results.
/// Output is capped at 2000 characters.
pub async fn build_ast_context(repo_path: &str, files: &[PrFile]) -> String {
    let repo_path = repo_path.to_string();
    let files: Vec<PrFile> = files.to_vec();

    match tokio::task::spawn_blocking(move || build_ast_context_sync(&repo_path, &files)).await {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("build_ast_context: blocking task panicked: {e}");
            String::new()
        }
    }
}

#[cfg(not(feature = "ast-context"))]
fn build_ast_context_sync(_repo_path: &str, _files: &[PrFile]) -> String {
    String::new()
}

#[cfg(feature = "ast-context")]
fn build_ast_context_sync(repo_path: &str, files: &[PrFile]) -> String {
    const CAP: usize = 2000;
    let mut output = String::from("\n<ast_context>\n");

    for file in files {
        if !is_rust_file(&file.filename) {
            continue;
        }
        let full_path = Path::new(repo_path).join(&file.filename);
        let path_str = full_path.to_string_lossy().to_string();

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
                if output.len() + file_block.len() > CAP {
                    break;
                }
                output.push_str(&file_block);
            }
            Err(e) => {
                debug!("ast_context: skipping {}: {}", file.filename, e);
            }
        }
    }
    output.push_str("</ast_context>\n");

    // If nothing was added (only the wrapper tags), return empty
    if output == "\n<ast_context>\n</ast_context>\n" {
        return String::new();
    }

    // Enforce cap on the full output
    if output.len() > CAP {
        let boundary = floor_char_boundary(&output, CAP);
        output.truncate(boundary);
        output.push_str("\n</ast_context>\n");
    }

    output
}

/// Build cross-file call graph context for the changed files.
///
/// For each function in each changed Rust file, looks up its callers.
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
        if !is_rust_file(&file.filename) {
            continue;
        }
        let full_path = repo.join(&file.filename);
        let path_str = full_path.to_string_lossy().to_string();

        // Get function names in this file
        let fn_names: Vec<String> = match analyze_file(&path_str, None) {
            Ok(a) => a
                .semantic
                .functions
                .iter()
                .map(|f| {
                    // compact_signature() returns "name(args) :line-line"
                    // Extract just the function name (before the '(')
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
            match analyze_focused(repo, fn_name, 1, Some(3), None) {
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
                                    .map(|n| n.to_string_lossy().to_string())
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
        }
    }

    #[tokio::test]
    async fn test_build_ast_context_missing_path_returns_empty() {
        let files = vec![make_pr_file("src/main.rs")];
        let result = build_ast_context("/nonexistent/path/xyz", &files).await;
        assert!(result.is_empty(), "expected empty for missing repo path");
    }

    #[tokio::test]
    async fn test_build_ast_context_valid_rust_file() {
        let repo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let files = vec![make_pr_file("src/ast_context.rs")];
        let result = build_ast_context(&repo_path, &files).await;
        // Verify it doesn't panic and respects the cap
        assert!(result.len() <= 2200, "output should be near cap");
    }

    #[tokio::test]
    async fn test_build_ast_context_cap_enforced() {
        let files: Vec<PrFile> = (0..50)
            .map(|i| make_pr_file(&format!("src/file_{i}.rs")))
            .collect();
        let result = build_ast_context(".", &files).await;
        assert!(
            result.len() <= 2200,
            "output must be capped near 2000 chars"
        );
    }
}
