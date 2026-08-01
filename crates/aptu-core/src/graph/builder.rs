// SPDX-License-Identifier: Apache-2.0

//! Builds a structural graph from source files via tree-sitter parsing.
//!
//! Currently only Rust (`.rs`) files are parsed for symbols and call edges.
//! Other file extensions produce a `File` node only, so additional language
//! grammars can be added without touching `cache.rs` or `query.rs`.

use std::collections::HashMap;
use std::path::Path;

use petgraph::graph::NodeIndex;

use super::{Edge, GraphDb, Node};

/// Builds a structural graph from a set of files.
///
/// Each entry is a `(path, content)` pair. Files with a `.rs` extension are
/// parsed with tree-sitter to extract function/struct/enum/trait/impl nodes
/// and calls/contains/`has_method`/implements edges. All other files receive
/// a single `File` node with no further structure.
///
/// Never panics on malformed input; parse failures are logged via
/// `tracing::warn!` and the offending file is skipped (its `File` node is
/// still added).
#[must_use]
pub fn build_graph(files: &[(&Path, &str)]) -> GraphDb {
    let mut graph = GraphDb::new();

    for (path, content) in files {
        let path_str = path.to_string_lossy().into_owned();
        let file_name = path
            .file_name()
            .map_or_else(|| path_str.clone(), |n| n.to_string_lossy().into_owned());

        let file_node = graph.add_node(Node::File {
            name: file_name,
            path: path_str.clone(),
        });

        if let Some("rs") = path.extension().and_then(|e| e.to_str()) {
            build_rust_file(&mut graph, file_node, &path_str, content);
            // Non-Rust files: File node only, no further structure (implicit else).
        }
    }

    graph
}

/// Parses a single Rust file and adds its symbols/edges to `graph`.
///
/// Skips the file (leaving only its `File` node) on any parse error, logging
/// via `tracing::warn!`. Never panics.
fn build_rust_file(graph: &mut GraphDb, file_node: NodeIndex, path: &str, content: &str) {
    let mut parser = tree_sitter::Parser::new();
    if let Err(e) = parser.set_language(&tree_sitter_rust::LANGUAGE.into()) {
        tracing::warn!(path, error = %e, "graph: failed to set tree-sitter Rust language");
        return;
    }

    let Some(tree) = parser.parse(content, None) else {
        tracing::warn!(path, "graph: tree-sitter failed to parse file");
        return;
    };

    let root = tree.root_node();
    if root.has_error() {
        tracing::warn!(
            path,
            "graph: parsed tree contains syntax errors; extracting best-effort symbols"
        );
    }

    // Map function name -> node index, for resolving call edges within the file.
    let mut function_nodes: HashMap<String, NodeIndex> = HashMap::new();

    let mut cursor = root.walk();
    walk_items(
        graph,
        file_node,
        path,
        content,
        &mut cursor,
        &mut function_nodes,
    );

    // Second pass: resolve call expressions within each function body to Calls edges.
    let mut cursor2 = root.walk();
    walk_calls(graph, path, content, &mut cursor2, &function_nodes, None);
}

/// Extracts visibility modifier text preceding a node, if present.
fn visibility_of(node: tree_sitter::Node, content: &str) -> String {
    if let Some(vis) = node.child_by_field_name("visibility_modifier") {
        content[vis.byte_range()].to_string()
    } else {
        "private".to_string()
    }
}

/// Extracts the `name` field of a node as a string, or empty string if absent.
fn name_of(node: tree_sitter::Node, content: &str) -> String {
    node.child_by_field_name("name")
        .map(|n| content[n.byte_range()].to_string())
        .unwrap_or_default()
}

/// Walks top-level and nested items, adding nodes to the graph and recording
/// `Contains` edges from the enclosing scope.
/// Walks methods inside an `impl` body node, adding `HasMethod` edges.
fn walk_impl_methods(
    graph: &mut GraphDb,
    impl_node: NodeIndex,
    body: tree_sitter::Node,
    path: &str,
    content: &str,
    function_nodes: &mut HashMap<String, NodeIndex>,
) {
    let mut method_cursor = body.walk();
    if method_cursor.goto_first_child() {
        loop {
            let m = method_cursor.node();
            if m.kind() == "function_item" {
                let mname = name_of(m, content);
                let mvis = visibility_of(m, content);
                let m_node = graph.add_node(Node::Function {
                    name: mname.clone(),
                    path: path.to_string(),
                    visibility: mvis,
                });
                graph.add_edge(impl_node, m_node, Edge::HasMethod);
                if !mname.is_empty() {
                    function_nodes.insert(mname, m_node);
                }
            }
            if !method_cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn walk_items(
    graph: &mut GraphDb,
    parent: NodeIndex,
    path: &str,
    content: &str,
    cursor: &mut tree_sitter::TreeCursor,
    function_nodes: &mut HashMap<String, NodeIndex>,
) {
    let node = cursor.node();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "function_item" => {
                    let name = name_of(child, content);
                    let visibility = visibility_of(child, content);
                    let fn_node = graph.add_node(Node::Function {
                        name: name.clone(),
                        path: path.to_string(),
                        visibility,
                    });
                    graph.add_edge(parent, fn_node, Edge::Contains);
                    if !name.is_empty() {
                        function_nodes.insert(name, fn_node);
                    }
                }
                "struct_item" => {
                    let name = name_of(child, content);
                    let visibility = visibility_of(child, content);
                    let s_node = graph.add_node(Node::Struct {
                        name,
                        path: path.to_string(),
                        visibility,
                    });
                    graph.add_edge(parent, s_node, Edge::Contains);
                }
                "enum_item" => {
                    let name = name_of(child, content);
                    let visibility = visibility_of(child, content);
                    let e_node = graph.add_node(Node::Enum {
                        name,
                        path: path.to_string(),
                        visibility,
                    });
                    graph.add_edge(parent, e_node, Edge::Contains);
                }
                "trait_item" => {
                    let name = name_of(child, content);
                    let visibility = visibility_of(child, content);
                    let t_node = graph.add_node(Node::Trait {
                        name,
                        path: path.to_string(),
                        visibility,
                    });
                    graph.add_edge(parent, t_node, Edge::Contains);
                }
                "impl_item" => {
                    let name = child
                        .child_by_field_name("type")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    let impl_node = graph.add_node(Node::Impl {
                        name,
                        path: path.to_string(),
                    });
                    graph.add_edge(parent, impl_node, Edge::Contains);
                    if let Some(body) = child.child_by_field_name("body") {
                        walk_impl_methods(graph, impl_node, body, path, content, function_nodes);
                    }
                }
                "mod_item" => {
                    let name = name_of(child, content);
                    let mod_node = graph.add_node(Node::Module {
                        name,
                        path: path.to_string(),
                    });
                    graph.add_edge(parent, mod_node, Edge::Contains);
                    let mut inner = child.walk();
                    walk_items(graph, mod_node, path, content, &mut inner, function_nodes);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    let _ = node;
}

/// Walks the tree looking for `call_expression` nodes and records `Calls`
/// edges from the enclosing function (if known) to the called function
/// (if it resolves to a known function node in this file).
fn walk_calls(
    graph: &mut GraphDb,
    path: &str,
    content: &str,
    cursor: &mut tree_sitter::TreeCursor,
    function_nodes: &HashMap<String, NodeIndex>,
    current_fn: Option<NodeIndex>,
) {
    let node = cursor.node();
    let mut enclosing = current_fn;
    if node.kind() == "function_item" {
        let name = name_of(node, content);
        enclosing = function_nodes.get(&name).copied().or(current_fn);
    }
    if let (true, Some(caller), Some(func_field)) = (
        node.kind() == "call_expression",
        enclosing,
        node.child_by_field_name("function"),
    ) {
        let callee_name = content[func_field.byte_range()].to_string();
        // Strip path prefixes like "self." or "Type::" leaving the final segment.
        let callee_name = callee_name
            .rsplit("::")
            .next()
            .unwrap_or(&callee_name)
            .rsplit('.')
            .next()
            .unwrap_or(&callee_name)
            .to_string();
        if let Some(&callee) = function_nodes.get(&callee_name) {
            graph.add_edge(caller, callee, Edge::Calls);
        }
    }

    if cursor.goto_first_child() {
        loop {
            walk_calls(graph, path, content, cursor, function_nodes, enclosing);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_build_graph_valid_rust_two_functions_produces_calls_edge() {
        // Arrange: two functions where one calls the other.
        let content = "fn bar() {}\nfn foo() { bar(); }\n";
        let path = Path::new("src/lib.rs");
        let files = [(path, content)];

        // Act
        let graph = build_graph(&files);

        // Assert: two Function nodes exist.
        let function_names: Vec<&str> = graph
            .node_weights()
            .filter_map(|n| match n {
                Node::Function { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(function_names.contains(&"foo"));
        assert!(function_names.contains(&"bar"));

        // Assert: a Calls edge exists from foo to bar.
        let has_calls_edge = graph
            .edge_indices()
            .any(|e| matches!(graph.edge_weight(e), Some(Edge::Calls)));
        assert!(has_calls_edge, "expected a Calls edge from foo to bar");
    }

    #[test]
    fn test_build_graph_malformed_rust_does_not_panic_returns_graph_with_file_node() {
        // Arrange: malformed Rust content (unbalanced braces).
        let content = "fn foo( { { { unclosed";
        let path = Path::new("src/broken.rs");
        let files = [(path, content)];

        // Act: must not panic.
        let graph = build_graph(&files);

        // Assert: at least the File node was added; no Function nodes required.
        let has_file_node = graph.node_weights().any(|n| matches!(n, Node::File { .. }));
        assert!(
            has_file_node,
            "malformed file should still produce a File node"
        );
    }

    #[test]
    fn test_build_graph_non_rust_file_produces_file_node_only() {
        // Arrange
        let content = "print('hello')";
        let path = Path::new("script.py");
        let files = [(path, content)];

        // Act
        let graph = build_graph(&files);

        // Assert: exactly one node (File), no Function/Struct/etc.
        assert_eq!(graph.node_count(), 1);
        assert!(matches!(
            graph.node_weights().next(),
            Some(Node::File { .. })
        ));
    }
}
