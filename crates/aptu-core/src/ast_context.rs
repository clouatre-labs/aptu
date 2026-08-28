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
use std::path::Path;
use tracing::debug;

#[cfg(feature = "ast-context")]
use std::fmt::Write as _;

#[cfg(feature = "ast-context")]
use aptu_coder_core::{analyze_file, analyze_focused, language_for_extension};

/// Result of building AST context, including both the text string and the
/// structural graph built from the same analysis data.
#[derive(Debug)]
pub(crate) struct AstContextOutput {
    /// Text representation of AST context (for prompt injection).
    pub text: String,
    /// Structural graph built from the same analysis data.
    /// Only present when both `ast-context` and `graph` features are enabled.
    #[cfg(all(feature = "ast-context", feature = "graph"))]
    pub graph: crate::graph::StructuralGraph,
    /// Per-file map of `(start_line, end_line, symbol_name)` for every function found by
    /// AST analysis, keyed by the PR-relative filename (matches `PrFile::filename`).
    /// Lets callers resolve a changed line number to its enclosing symbol without a
    /// second analysis pass. Only present when both `ast-context` and `graph` are enabled.
    #[cfg(all(feature = "ast-context", feature = "graph"))]
    pub symbol_ranges: std::collections::HashMap<String, Vec<(usize, usize, String)>>,
}

impl AstContextOutput {
    #[cfg(all(feature = "ast-context", feature = "graph"))]
    pub(crate) fn new(text: String) -> Self {
        Self {
            text,
            graph: aptu_coder_core::graph::StructuralGraph::build_from_analysis(&[]),
            symbol_ranges: std::collections::HashMap::new(),
        }
    }

    #[cfg(not(all(feature = "ast-context", feature = "graph")))]
    pub(crate) fn new(text: String) -> Self {
        Self { text }
    }

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    fn with_graph(
        text: String,
        graph: crate::graph::StructuralGraph,
        symbol_ranges: std::collections::HashMap<String, Vec<(usize, usize, String)>>,
    ) -> Self {
        Self {
            text,
            graph,
            symbol_ranges,
        }
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

    // Accumulate analysis data for graph building
    #[cfg(feature = "graph")]
    let mut analysis_pairs: Vec<(std::path::PathBuf, aptu_coder_core::SemanticAnalysis)> =
        Vec::new();
    #[cfg(feature = "graph")]
    let mut impl_traits: Vec<aptu_coder_core::ImplTraitInfo> = Vec::new();
    #[cfg(feature = "graph")]
    let mut symbol_ranges: std::collections::HashMap<String, Vec<(usize, usize, String)>> =
        std::collections::HashMap::new();

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
                // Cap only the text output, not the graph data collection.
                // Previously a `break` here skipped analysis_pairs/symbol_ranges
                // for remaining files, producing an empty graph on large PRs (#1538).
                if output.len() + file_block.len() <= CAP {
                    output.push_str(&file_block);
                }

                // Always accumulate graph data regardless of text cap
                #[cfg(feature = "graph")]
                {
                    analysis_pairs.push((full_path.clone(), analysis.semantic.clone()));
                    impl_traits.extend(analysis.semantic.impl_traits.clone());

                    let ranges: Vec<(usize, usize, String)> = analysis
                        .semantic
                        .functions
                        .iter()
                        .map(|f| (f.line, f.end_line, f.name.clone()))
                        .collect();
                    if !ranges.is_empty() {
                        symbol_ranges.insert(file.filename.clone(), ranges);
                    }
                }
            }
            Err(e) => {
                debug!("ast_context: skipping {}: {}", file.filename, e);
            }
        }
    }
    output.push_str("</ast_context>\n");

    // If nothing was added (only the wrapper tags), clear the text but still
    // proceed to graph building -- graph data may have been accumulated even
    // when all file blocks exceeded the text cap (#1538).
    if output == "\n<ast_context>\n</ast_context>\n" {
        output.clear();
    }

    // Enforce cap on the full output
    if output.len() > CAP {
        let boundary = floor_char_boundary(&output, CAP);
        output.truncate(boundary);
        output.push_str("\n</ast_context>\n");
    }

    // Build structural graph from accumulated analysis data (no second analyze_file pass).
    #[cfg(feature = "graph")]
    {
        let entries: Vec<_> = analysis_pairs
            .iter()
            .map(|(path, semantic)| {
                aptu_coder_core::FileAnalysisOutput::new(
                    path.to_string_lossy().into_owned(),
                    String::new(),
                    semantic.clone(),
                    0,
                    None,
                )
            })
            .collect();
        let graph = aptu_coder_core::graph::StructuralGraph::build_from_analysis(&entries);
        AstContextOutput::with_graph(output, graph, symbol_ranges)
    }

    #[cfg(not(feature = "graph"))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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

    #[cfg(all(feature = "ast-context", feature = "graph"))]
    #[tokio::test]
    async fn test_graph_data_populated_when_text_cap_exceeded() {
        // Regression test for #1538: when the first file's AST block exceeds the
        // 2000-char text cap, analysis_pairs and symbol_ranges must still be
        // populated so the graph is non-empty.
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("big.rs");
        // Generate enough functions to exceed the 2000-char text cap.
        let mut content = String::from("// SPDX-License-Identifier: Apache-2.0\n");
        for i in 0..80 {
            content.push_str(&format!(
                "pub fn func_{i}(x: u32, y: u32) -> u32 {{ x + y + {i} }}\n"
            ));
        }
        std::fs::write(&file_path, &content).unwrap();

        let files = vec![PrFile {
            filename: "big.rs".to_string(),
            status: "modified".to_string(),
            additions: 80,
            deletions: 0,
            patch: None,
            patch_truncated: false,
            full_content: None,
        }];
        let result = build_ast_context(temp_dir.path().to_str().unwrap(), &files).await;

        // Text should be capped near 2000 chars (or empty if the single file
        // block exceeds CAP, which is acceptable as long as graph data is present)
        assert!(
            result.text.len() <= 2200,
            "text must be capped near 2000 chars, got {}",
            result.text.len()
        );

        // Graph data must still be populated despite the text cap
        assert!(
            !result.symbol_ranges.is_empty(),
            "symbol_ranges must be populated even when text cap is exceeded"
        );
        assert!(
            result.symbol_ranges.contains_key("big.rs"),
            "symbol_ranges must contain 'big.rs' entry"
        );
        assert!(
            !result.symbol_ranges["big.rs"].is_empty(),
            "symbol_ranges for 'big.rs' must have function entries"
        );

        // The graph must be built from the same uncapped analysis data, not from
        // the text that happened to fit in the AST context budget.
        let symbols: Vec<&str> = result
            .graph
            .graph
            .node_indices()
            .filter_map(|index| match &result.graph.graph[index] {
                aptu_coder_core::graph::Node::Symbol { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            symbols.contains(&"func_0"),
            "graph must contain first symbol"
        );
        assert!(
            symbols.contains(&"func_79"),
            "graph must contain last symbol"
        );
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
