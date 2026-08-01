// SPDX-License-Identifier: Apache-2.0

//! Builds a structural graph by parsing the rendered `<ast_context>` and
//! `<call_graph>` string blocks already produced by
//! `crate::ast_context::build_ast_context` /
//! `crate::ast_context::build_call_graph_context`.
//!
//! # Design constraint
//!
//! This module MUST NOT introduce a second AST parse pass, must not add a
//! native grammar crate as a dependency, and must not re-read source files.
//! It consumes only the rendered strings that `ast_context.rs` already
//! produced for the LLM prompt.
//!
//! # Format contract (pinned to aptu-coder-core v0.26.1)
//!
//! `<ast_context>` block lines:
//! - `"## <filename>"` — start of a new file section; emits a `File` node.
//! - Lines starting with `"  fn "` — `Function` node (`compact_signature` format).
//! - Lines starting with `"  imports: "` — `Imports` edges from the enclosing `File` node.
//!
//! `<call_graph>` block lines:
//! - Lines starting with `"### callers of"` — opens a caller block for the named function.
//! - Lines starting with two spaces — a `Calls` edge: caller → callee.
//!
//! If aptu-coder-core changes its `compact_signature()` layout or caller-block
//! format, update this parser to match.

use std::collections::HashMap;

use petgraph::graph::NodeIndex;

use super::{Edge, GraphDb, Node};

/// Parses the rendered `<ast_context>` string and returns a [`GraphDb`].
///
/// Handles truncated output (missing closing tag) and empty input gracefully.
/// Never panics on malformed lines; unrecognised lines are skipped silently.
#[must_use]
pub fn parse_ast_context_string(ast: &str) -> GraphDb {
    let mut graph = GraphDb::new();
    // Maps file path → NodeIndex for Imports-edge lookups.
    let mut file_nodes: HashMap<String, NodeIndex> = HashMap::new();

    let mut current_file: Option<NodeIndex> = None;
    let mut current_file_path = String::new();
    let mut inside_block = false;

    for raw_line in ast.lines() {
        let line = raw_line.trim_end_matches('\r');

        match line {
            "<ast_context>" => {
                inside_block = true;
                continue;
            }
            "</ast_context>" => {
                inside_block = false;
                continue;
            }
            _ => {}
        }

        if !inside_block {
            continue;
        }

        // File header: "## <filename>" (no leading spaces)
        if let Some(filename) = line.strip_prefix("## ") {
            let filename = filename.trim().to_string();
            let file_idx = graph.add_node(Node::File {
                name: filename.clone(),
                path: filename.clone(),
            });
            file_nodes.insert(filename.clone(), file_idx);
            current_file = Some(file_idx);
            current_file_path = filename;
            continue;
        }

        // Function line: "  fn name(params) -> ret :start-end"
        if let Some(rest) = line.strip_prefix("  fn ") {
            let fn_name = rest.split('(').next().unwrap_or("").trim().to_string();
            if fn_name.is_empty() {
                continue;
            }
            // Determine visibility from name prefix (pub fn → strip; otherwise private).
            let visibility = if fn_name.starts_with("pub ") {
                "pub".to_string()
            } else {
                "private".to_string()
            };
            let clean_name = fn_name.strip_prefix("pub ").unwrap_or(&fn_name).to_string();

            let fn_idx = graph.add_node(Node::Function {
                name: clean_name.clone(),
                path: current_file_path.clone(),
                visibility,
            });
            if let Some(file_idx) = current_file {
                graph.add_edge(file_idx, fn_idx, Edge::Contains);
            }
            continue;
        }

        // Imports line: "  imports: mod1 mod2 mod3"
        if let (Some(imports_str), Some(file_idx)) =
            (line.strip_prefix("  imports: "), current_file)
        {
            for import in imports_str.split_whitespace() {
                let import_node = graph.add_node(Node::Module {
                    name: import.to_string(),
                    path: String::new(),
                });
                graph.add_edge(file_idx, import_node, Edge::Imports);
            }
        }
    }

    graph
}

/// Adds `Calls` edges to `graph` by parsing the rendered `<call_graph>` string.
///
/// For each "callers of `fn_name`" block, looks up `fn_name` in the existing
/// graph nodes (by `Node::name()`) and adds a `Calls` edge from each listed
/// caller to `fn_name`. Caller nodes not already in the graph are added as
/// anonymous `Function` nodes.
///
/// Handles truncated output and empty input gracefully.
pub fn parse_call_graph_string(call_graph: &str, graph: &mut GraphDb) {
    // Build an index of existing function nodes by name for O(1) lookup.
    let mut name_to_idx: HashMap<String, NodeIndex> = graph
        .node_indices()
        .filter_map(|idx| {
            if matches!(graph[idx], Node::Function { .. }) {
                Some((graph[idx].name().to_string(), idx))
            } else {
                None
            }
        })
        .collect();

    let mut current_callee: Option<NodeIndex> = None;
    let mut inside_block = false;

    for raw_line in call_graph.lines() {
        let line = raw_line.trim_end_matches('\r');

        match line {
            "<call_graph>" => {
                inside_block = true;
                continue;
            }
            "</call_graph>" => {
                inside_block = false;
                current_callee = None;
                continue;
            }
            _ => {}
        }

        if !inside_block {
            continue;
        }

        // Callee header: "### callers of `fn_name`"
        if let Some(rest) = line.strip_prefix("### callers of `") {
            let fn_name = rest.trim_end_matches('`').trim().to_string();
            if fn_name.is_empty() {
                continue;
            }
            // Find or create the callee node.
            let callee_idx = if let Some(&idx) = name_to_idx.get(&fn_name) {
                idx
            } else {
                let idx = graph.add_node(Node::Function {
                    name: fn_name.clone(),
                    path: String::new(),
                    visibility: "private".to_string(),
                });
                name_to_idx.insert(fn_name, idx);
                idx
            };
            current_callee = Some(callee_idx);
            continue;
        }

        // Caller line: "  caller_sym (file:line)" — exactly two leading spaces
        if line.starts_with("  ")
            && !line.starts_with("   ")
            && let Some(callee_idx) = current_callee
        {
            let caller_name = line.split_whitespace().next().unwrap_or("").to_string();
            if caller_name.is_empty() {
                continue;
            }
            let caller_idx = if let Some(&idx) = name_to_idx.get(&caller_name) {
                idx
            } else {
                let idx = graph.add_node(Node::Function {
                    name: caller_name.clone(),
                    path: String::new(),
                    visibility: "private".to_string(),
                });
                name_to_idx.insert(caller_name, idx);
                idx
            };
            graph.add_edge(caller_idx, callee_idx, Edge::Calls);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ast_context(lines: &[&str]) -> String {
        let mut s = "<ast_context>\n".to_string();
        for l in lines {
            s.push_str(l);
            s.push('\n');
        }
        s.push_str("</ast_context>\n");
        s
    }

    fn make_call_graph(lines: &[&str]) -> String {
        let mut s = "<call_graph>\n".to_string();
        for l in lines {
            s.push_str(l);
            s.push('\n');
        }
        s.push_str("</call_graph>\n");
        s
    }

    #[test]
    fn test_parse_ast_context_function_node() {
        // Arrange: a single function in a file.
        let ast = make_ast_context(&[
            "## src/lib.rs",
            "  fn apply_changes(repo: &Repo) -> Result<()> :10-45",
        ]);

        // Act
        let graph = parse_ast_context_string(&ast);

        // Assert: one File node and one Function node.
        let fn_names: Vec<&str> = graph
            .node_weights()
            .filter_map(|n| match n {
                Node::Function { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            fn_names.contains(&"apply_changes"),
            "expected function node 'apply_changes'; got {fn_names:?}"
        );
        let file_count = graph
            .node_weights()
            .filter(|n| matches!(n, Node::File { .. }))
            .count();
        assert_eq!(file_count, 1, "expected one File node");
    }

    #[test]
    fn test_parse_ast_context_imports_edges() {
        // Arrange
        let ast = make_ast_context(&["## src/lib.rs", "  imports: std::io std::fmt"]);

        // Act
        let graph = parse_ast_context_string(&ast);

        // Assert: two Module nodes (one per import) and two Imports edges.
        let module_count = graph
            .node_weights()
            .filter(|n| matches!(n, Node::Module { .. }))
            .count();
        assert_eq!(module_count, 2, "expected two Module nodes from imports");
        let imports_edge_count = graph
            .edge_indices()
            .filter(|&e| matches!(graph.edge_weight(e), Some(Edge::Imports)))
            .count();
        assert_eq!(imports_edge_count, 2, "expected two Imports edges");
    }

    #[test]
    fn test_parse_ast_context_empty_string_returns_empty_graph() {
        // Arrange: only tag wrappers (or completely empty).
        let ast = "<ast_context>\n</ast_context>\n";

        // Act
        let graph = parse_ast_context_string(ast);

        // Assert
        assert_eq!(
            graph.node_count(),
            0,
            "empty block should produce empty graph"
        );
    }

    #[test]
    fn test_parse_ast_context_missing_closing_tag_does_not_panic() {
        // Arrange: truncated at the 2000-char cap — no closing tag.
        let ast = "<ast_context>\n## src/lib.rs\n  fn foo() -> () :1-5\n";

        // Act: must not panic.
        let graph = parse_ast_context_string(ast);

        // Assert: Function node was still extracted.
        let has_fn = graph
            .node_weights()
            .any(|n| matches!(n, Node::Function { name, .. } if name == "foo"));
        assert!(has_fn, "truncated input should still yield Function node");
    }

    #[test]
    fn test_parse_call_graph_adds_calls_edges() {
        // Arrange: two callers for target.
        let mut graph = GraphDb::new();
        graph.add_node(Node::Function {
            name: "target".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "pub".to_string(),
        });

        let cg = make_call_graph(&[
            "### callers of `target`",
            "  caller_a (src/a.rs:10)",
            "  caller_b (src/b.rs:20)",
        ]);

        // Act
        parse_call_graph_string(&cg, &mut graph);

        // Assert: a Calls edge exists from each caller to target.
        let calls_count = graph
            .edge_indices()
            .filter(|&e| matches!(graph.edge_weight(e), Some(Edge::Calls)))
            .count();
        assert_eq!(
            calls_count, 2,
            "expected two Calls edges; got {calls_count}"
        );

        let caller_names: Vec<&str> = graph
            .node_weights()
            .filter_map(|n| match n {
                Node::Function { name, .. } if name != "target" => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(caller_names.contains(&"caller_a"));
        assert!(caller_names.contains(&"caller_b"));
    }

    #[test]
    fn test_parse_ast_context_skips_unrecognised_lines() {
        // Arrange: valid lines mixed with unrecognised prefixes and blank lines.
        // Documents the dependency on the aptu-coder-core compact_signature format.
        // If that format changes, this test will fail and alert maintainers.
        let ast = make_ast_context(&[
            "## src/lib.rs",
            "  fn valid_fn() -> () :1-5",
            "",
            "  UNKNOWN_PREFIX something",
            "  struct Foo :7-12",
            "  fn another_fn(x: u32) -> u32 :20-30",
        ]);

        // Act: unrecognised lines must be silently skipped.
        let graph = parse_ast_context_string(&ast);

        // Assert: only the two recognised Function nodes were added.
        let fn_names: Vec<&str> = graph
            .node_weights()
            .filter_map(|n| match n {
                Node::Function { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            fn_names.contains(&"valid_fn"),
            "expected 'valid_fn'; got {fn_names:?}"
        );
        assert!(
            fn_names.contains(&"another_fn"),
            "expected 'another_fn'; got {fn_names:?}"
        );
        // 'UNKNOWN_PREFIX something' must not create a spurious node.
        assert_eq!(
            fn_names.len(),
            2,
            "expected exactly 2 function nodes; got {fn_names:?}"
        );
    }

    #[test]
    fn test_parse_call_graph_empty_returns_unchanged_graph() {
        // Arrange
        let mut graph = GraphDb::new();
        graph.add_node(Node::Function {
            name: "foo".to_string(),
            path: "".to_string(),
            visibility: "pub".to_string(),
        });
        let before_nodes = graph.node_count();
        let before_edges = graph.edge_count();

        // Act: empty block.
        let cg = "<call_graph>\n</call_graph>\n";
        parse_call_graph_string(cg, &mut graph);

        // Assert: graph unchanged.
        assert_eq!(graph.node_count(), before_nodes);
        assert_eq!(graph.edge_count(), before_edges);
    }
}
