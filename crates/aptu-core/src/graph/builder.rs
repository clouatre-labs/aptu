// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2025 Agentic AI Foundation

//! Builds a structural graph directly from typed aptu-coder-core analysis
//! structs, bypassing the text-format parser that was previously required.
//!
//! # Design constraint
//!
//! This module MUST NOT introduce a second AST parse pass, must not add a
//! native grammar crate as a dependency, and must not re-read source files.
//! It consumes only the typed analysis data that `ast_context.rs` already
//! produced via `aptu_coder_core::analyze_file`.

use std::collections::HashMap;

use petgraph::graph::NodeIndex;

use super::{Edge, GraphDb, Node};

/// Builds a [`GraphDb`] from typed aptu-coder-core analysis structs.
///
/// Emits:
/// - `Node::File` for the given `path`
/// - `Node::Function` for each function in `semantic.functions`
/// - `Edge::Contains` from the File node to each Function node
/// - `Node::Module` + `Edge::Imports` for each entry in `semantic.imports`
/// - `Edge::Calls` from `call_graph.callers`, filtering out pseudo-edges
///   (`neighbor_name == "<reference>"`) and impl-trait edges
///   (`is_impl_trait == true`)
///
/// All function nodes default to `visibility = "private"` because
/// `FunctionInfo` has no visibility field.
///
/// `CallEdge.path` is normalized to an empty string for anonymous callers.
#[cfg(all(feature = "ast-context", feature = "graph"))]
#[must_use]
pub fn build_from_analysis(
    path: &str,
    semantic: &aptu_coder_core::SemanticAnalysis,
    call_graph: &aptu_coder_core::graph::CallGraph,
) -> GraphDb {
    let mut graph = GraphDb::new();

    // File node
    let file_idx = graph.add_node(Node::File {
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        path: path.to_string(),
    });

    // Function nodes + Contains edges
    let mut fn_name_to_idx: HashMap<String, NodeIndex> = HashMap::new();
    for func in &semantic.functions {
        let fn_idx = graph.add_node(Node::Function {
            name: func.name.clone(),
            path: path.to_string(),
            visibility: "private".to_string(),
        });
        graph.add_edge(file_idx, fn_idx, Edge::Contains);
        fn_name_to_idx.insert(func.name.clone(), fn_idx);
    }

    // Module nodes + Imports edges
    for imp in &semantic.imports {
        let module_idx = graph.add_node(Node::Module {
            name: imp.module.clone(),
            path: String::new(),
        });
        graph.add_edge(file_idx, module_idx, Edge::Imports);
    }

    // Calls edges from call_graph.callers
    // Filter: skip <reference> pseudo-edges and impl-trait edges
    for (callee_name, call_edges) in &call_graph.callers {
        // Find or create callee node
        let dst = fn_name_to_idx.get(callee_name).copied().unwrap_or_else(|| {
            let idx = graph.add_node(Node::Function {
                name: callee_name.clone(),
                path: String::new(),
                visibility: "private".to_string(),
            });
            fn_name_to_idx.insert(callee_name.clone(), idx);
            idx
        });

        for call_edge in call_edges {
            // Skip pseudo-edges
            if call_edge.neighbor_name == "<reference>" {
                continue;
            }
            // Skip impl-trait edges for parity with today's parser
            if call_edge.is_impl_trait {
                continue;
            }

            let raw_path = call_edge.path.to_string_lossy().into_owned();
            // Normalize: empty string for anonymous callers
            let caller_path = if raw_path.is_empty() || raw_path == "." {
                String::new()
            } else {
                raw_path
            };

            let src = fn_name_to_idx
                .get(&call_edge.neighbor_name)
                .copied()
                .unwrap_or_else(|| {
                    let idx = graph.add_node(Node::Function {
                        name: call_edge.neighbor_name.clone(),
                        path: caller_path,
                        visibility: "private".to_string(),
                    });
                    fn_name_to_idx.insert(call_edge.neighbor_name.clone(), idx);
                    idx
                });

            graph.add_edge(src, dst, Edge::Calls);
        }
    }

    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    use aptu_coder_core::graph::CallGraph;
    use aptu_coder_core::{CallEdge, FunctionInfo, ImportInfo, SemanticAnalysis};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_fn(name: &str) -> FunctionInfo {
        let mut f = FunctionInfo::default();
        f.name = name.to_string();
        f
    }

    fn make_fn_with_params(name: &str, params: Vec<&str>, ret: Option<&str>) -> FunctionInfo {
        let mut f = FunctionInfo::default();
        f.name = name.to_string();
        f.line = 1;
        f.end_line = 10;
        f.parameters = params.into_iter().map(str::to_string).collect();
        f.return_type = ret.map(str::to_string);
        f
    }

    fn make_semantic(functions: Vec<FunctionInfo>, imports: Vec<ImportInfo>) -> SemanticAnalysis {
        SemanticAnalysis::new(
            functions,
            vec![],
            imports,
            vec![],
            HashMap::new(),
            vec![],
            vec![],
        )
    }

    fn make_call_graph(callers: HashMap<String, Vec<CallEdge>>) -> CallGraph {
        let mut cg = CallGraph::new();
        cg.callers = callers;
        cg
    }

    #[test]
    fn test_build_from_analysis_emits_file_function_contains() {
        // Arrange: one function in one file
        let semantic = make_semantic(
            vec![make_fn_with_params(
                "apply_changes",
                vec!["repo: &Repo"],
                Some("Result<()>"),
            )],
            vec![],
        );
        let call_graph = make_call_graph(HashMap::new());

        // Act
        let graph = build_from_analysis("src/lib.rs", &semantic, &call_graph);

        // Assert: one File node, one Function node, one Contains edge
        let file_count = graph
            .node_weights()
            .filter(|n| matches!(n, Node::File { .. }))
            .count();
        assert_eq!(file_count, 1, "expected one File node");

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

        let contains_count = graph
            .edge_indices()
            .filter(|&e| matches!(graph.edge_weight(e), Some(Edge::Contains)))
            .count();
        assert_eq!(contains_count, 1, "expected one Contains edge");
    }

    #[test]
    fn test_build_from_analysis_filters_reference_edges() {
        // Arrange: a callee with one valid caller and one <reference> pseudo-edge
        let semantic = make_semantic(vec![make_fn("target_fn")], vec![]);
        let mut callers: HashMap<String, Vec<CallEdge>> = HashMap::new();
        callers.insert(
            "target_fn".to_string(),
            vec![
                CallEdge {
                    neighbor_name: "real_caller".to_string(),
                    path: PathBuf::from("src/caller.rs"),
                    line: 10,
                    is_impl_trait: false,
                },
                CallEdge {
                    neighbor_name: "<reference>".to_string(),
                    path: PathBuf::from("src/other.rs"),
                    line: 20,
                    is_impl_trait: false,
                },
            ],
        );
        let call_graph = make_call_graph(callers);

        // Act
        let graph = build_from_analysis("src/lib.rs", &semantic, &call_graph);

        // Assert: only one Calls edge (real_caller -> target_fn), <reference> filtered out
        let calls_count = graph
            .edge_indices()
            .filter(|&e| matches!(graph.edge_weight(e), Some(Edge::Calls)))
            .count();
        assert_eq!(
            calls_count, 1,
            "expected one Calls edge; <reference> must be filtered"
        );

        let caller_names: Vec<&str> = graph
            .node_weights()
            .filter_map(|n| match n {
                Node::Function { name, .. } if name != "target_fn" => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            caller_names.contains(&"real_caller"),
            "expected 'real_caller' node"
        );
        assert!(
            !caller_names.contains(&"<reference>"),
            "<reference> must not appear as a node"
        );
    }

    #[test]
    fn test_build_from_analysis_empty_produces_empty_graph() {
        // Arrange: empty semantic, empty call_graph
        let semantic = make_semantic(vec![], vec![]);
        let call_graph = make_call_graph(HashMap::new());

        // Act
        let graph = build_from_analysis("src/empty.rs", &semantic, &call_graph);

        // Assert: only the File node exists
        assert_eq!(graph.node_count(), 1, "expected only the File node");
        let file_count = graph
            .node_weights()
            .filter(|n| matches!(n, Node::File { .. }))
            .count();
        assert_eq!(file_count, 1, "expected one File node");
        assert_eq!(graph.edge_count(), 0, "expected no edges");
    }

    #[test]
    fn test_build_from_analysis_defaults_visibility_to_private() {
        // Arrange: a function with no visibility info
        let semantic = make_semantic(vec![make_fn("helper")], vec![]);
        let call_graph = make_call_graph(HashMap::new());

        // Act
        let graph = build_from_analysis("src/lib.rs", &semantic, &call_graph);

        // Assert: visibility defaults to "private"
        let visibilities: Vec<&str> = graph
            .node_weights()
            .filter_map(|n| match n {
                Node::Function { visibility, .. } => Some(visibility.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(visibilities.len(), 1, "expected one function node");
        assert_eq!(
            visibilities[0], "private",
            "visibility should default to 'private'"
        );
    }
}
