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

#[cfg(all(feature = "ast-context", feature = "graph"))]
use aptu_coder_core::{SemanticAnalysis, graph::CallGraph};

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
    semantic: &SemanticAnalysis,
    call_graph: &CallGraph,
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

#[cfg(all(test, feature = "ast-context", feature = "graph"))]
mod tests {
    use super::*;

    use aptu_coder_core::FileAnalysisOutput;
    use aptu_coder_core::analyze_str;
    use aptu_coder_core::graph::CallGraph;
    use aptu_coder_core::graph::StructuralGraph;
    use aptu_coder_core::graph::{Edge as CoderEdge, Node as CoderNode};
    use aptu_coder_core::{CallEdge, FunctionInfo, ImportInfo, SemanticAnalysis};
    use std::collections::{HashMap, HashSet};
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

    /// Build a CallGraph from real `semantic.calls` data for a given file path.
    fn make_call_graph_from_semantic(semantic: &SemanticAnalysis, file_path: &str) -> CallGraph {
        let mut callers: HashMap<String, Vec<CallEdge>> = HashMap::new();
        for call in &semantic.calls {
            callers
                .entry(call.callee.clone())
                .or_default()
                .push(CallEdge {
                    neighbor_name: call.caller.clone(),
                    path: PathBuf::from(file_path),
                    line: call.line,
                    is_impl_trait: false,
                });
        }
        make_call_graph(callers)
    }

    /// Collect Calls edges as (caller, callee) name-pairs from aptu's GraphDb.
    fn collect_aptu_calls(graph: &GraphDb) -> HashSet<(String, String)> {
        graph
            .edge_indices()
            .filter_map(|e| {
                let (src, dst) = graph.edge_endpoints(e)?;
                if !matches!(graph.edge_weight(e), Some(Edge::Calls)) {
                    return None;
                }
                let Node::Function { name: src_name, .. } = &graph[src] else {
                    return None;
                };
                let Node::Function { name: dst_name, .. } = &graph[dst] else {
                    return None;
                };
                Some((src_name.clone(), dst_name.clone()))
            })
            .collect()
    }

    /// Collect Calls edges as (caller, callee) name-pairs from StructuralGraph.
    fn collect_structural_calls(graph: &StructuralGraph) -> HashSet<(String, String)> {
        graph
            .graph
            .edge_indices()
            .filter_map(|e| {
                let (src, dst) = graph.graph.edge_endpoints(e)?;
                if !matches!(graph.graph.edge_weight(e), Some(CoderEdge::Calls)) {
                    return None;
                }
                let CoderNode::Symbol { name: src_name, .. } = &graph.graph[src] else {
                    return None;
                };
                let CoderNode::Symbol { name: dst_name, .. } = &graph.graph[dst] else {
                    return None;
                };
                Some((src_name.clone(), dst_name.clone()))
            })
            .collect()
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
    fn test_calls_parity_with_structural_graph() {
        // CallInfo/ReferenceInfo/ImplTraitInfo are #[non_exhaustive] in
        // aptu-coder-core with no public constructor, so a real `.calls`
        // entry must come from the crate's own analyzer, not a hand-built
        // literal.
        let source = "fn callee_fn() {}\nfn caller_fn() { callee_fn(); }\n";
        let analyzed = analyze_str(source, "rust", None).expect("analyze_str");
        let semantic = analyzed.semantic;
        assert_eq!(
            semantic.calls.len(),
            1,
            "fixture must produce exactly one call"
        );

        // aptu-core's CallGraph is hand-built (as in the tests above) so it
        // can carry `<reference>` and impl-trait synthetic edges alongside
        // the real one, exercising builder.rs's filter.
        let mut callers: HashMap<String, Vec<CallEdge>> = HashMap::new();
        callers.insert(
            "callee_fn".to_string(),
            vec![
                CallEdge {
                    neighbor_name: "caller_fn".to_string(),
                    path: PathBuf::from("src/lib.rs"),
                    line: 2,
                    is_impl_trait: false,
                },
                CallEdge {
                    neighbor_name: "<reference>".to_string(),
                    path: PathBuf::from("src/lib.rs"),
                    line: 2,
                    is_impl_trait: false,
                },
                CallEdge {
                    neighbor_name: "SomeImpl".to_string(),
                    path: PathBuf::from("src/lib.rs"),
                    line: 2,
                    is_impl_trait: true,
                },
            ],
        );
        let call_graph = make_call_graph(callers);

        // Act: aptu-core's CallGraph-based builder.
        let aptu_graph = build_from_analysis("src/lib.rs", &semantic, &call_graph);
        let aptu_calls = collect_aptu_calls(&aptu_graph);

        // Act: aptu-coder-core's StructuralGraph, fed the SAME real
        // semantic — it only ever reads `semantic.calls`, never
        // `.references`/`.impl_traits`.
        let entries = vec![FileAnalysisOutput::new(
            "src/lib.rs".to_string(),
            String::new(),
            semantic,
            source.lines().count(),
            None,
        )];
        let structural = StructuralGraph::build_from_analysis(&entries);
        let structural_calls = collect_structural_calls(&structural);

        // Assert: both builders agree on the real edge; the `<reference>`
        // pseudo-edge and impl-trait edge that aptu-core must filter never
        // appear in StructuralGraph's output because they were never part
        // of `semantic.calls` to begin with.
        let expected: HashSet<(String, String)> =
            [("caller_fn".to_string(), "callee_fn".to_string())]
                .into_iter()
                .collect();
        assert_eq!(aptu_calls, expected, "aptu-core builder Calls edges");
        assert_eq!(
            structural_calls, expected,
            "StructuralGraph builder Calls edges"
        );
    }

    /// Cross-file same-name collision fixture.
    ///
    /// Starting with aptu-coder-core 0.32.0, StructuralGraph::build_from_analysis
    /// resolves cross-file same-name collisions via same-file preference.
    /// In contrast, aptu's build_from_analysis is a per-file builder.
    /// Both builders agree on name-pair output, while node-level resolution
    /// differs (StructuralGraph resolves within the same file).
    #[test]
    fn test_cross_file_calls_parity_with_structural_graph() {
        let src_a = "fn helper() {}\nfn main() { helper(); }\n";
        let src_b = "fn helper() {}\nfn main() { helper(); }\n";

        let semantic_a = analyze_str(src_a, "rust", None)
            .expect("analyze_str src_a")
            .semantic;
        let semantic_b = analyze_str(src_b, "rust", None)
            .expect("analyze_str src_b")
            .semantic;

        assert_eq!(semantic_a.calls.len(), 1, "src_a must produce one call");
        assert_eq!(semantic_b.calls.len(), 1, "src_b must produce one call");

        // Act: aptu-core builder (per-file)
        let call_graph_a = make_call_graph_from_semantic(&semantic_a, "src/a.rs");
        let call_graph_b = make_call_graph_from_semantic(&semantic_b, "src/b.rs");
        let aptu_graph_a = build_from_analysis("src/a.rs", &semantic_a, &call_graph_a);
        let aptu_graph_b = build_from_analysis("src/b.rs", &semantic_b, &call_graph_b);

        let aptu_calls_a = collect_aptu_calls(&aptu_graph_a);
        let aptu_calls_b = collect_aptu_calls(&aptu_graph_b);
        let aptu_calls_a_count = aptu_graph_a
            .edge_indices()
            .filter(|e| matches!(aptu_graph_a.edge_weight(*e), Some(Edge::Calls)))
            .count();
        let aptu_calls_b_count = aptu_graph_b
            .edge_indices()
            .filter(|e| matches!(aptu_graph_b.edge_weight(*e), Some(Edge::Calls)))
            .count();

        let mut aptu_cross_calls = aptu_calls_a;
        aptu_cross_calls.extend(aptu_calls_b);

        // Act: aptu-coder-core StructuralGraph with both files
        let cross_entries = vec![
            FileAnalysisOutput::new(
                "src/a.rs".to_string(),
                String::new(),
                semantic_a.clone(),
                src_a.lines().count(),
                None,
            ),
            FileAnalysisOutput::new(
                "src/b.rs".to_string(),
                String::new(),
                semantic_b.clone(),
                src_b.lines().count(),
                None,
            ),
        ];
        let structural_cross = StructuralGraph::build_from_analysis(&cross_entries);
        let structural_cross_calls = collect_structural_calls(&structural_cross);

        let structural_calls_count = structural_cross
            .graph
            .edge_indices()
            .filter(|e| {
                matches!(
                    structural_cross.graph.edge_weight(*e),
                    Some(CoderEdge::Calls)
                )
            })
            .count();

        let mut edges_cross_files = false;
        for e in structural_cross.graph.edge_indices() {
            if matches!(
                structural_cross.graph.edge_weight(e),
                Some(CoderEdge::Calls)
            ) {
                if let Some((src, dst)) = structural_cross.graph.edge_endpoints(e) {
                    let (
                        CoderNode::Symbol {
                            file_path: src_file,
                            ..
                        },
                        CoderNode::Symbol {
                            file_path: dst_file,
                            ..
                        },
                    ) = (&structural_cross.graph[src], &structural_cross.graph[dst])
                    else {
                        continue;
                    };
                    if src_file != dst_file {
                        edges_cross_files = true;
                    }
                }
            }
        }

        // Assert:
        // 1. StructuralGraph produces exactly 2 Calls edges
        assert_eq!(
            structural_calls_count, 2,
            "StructuralGraph must produce exactly 2 Calls edges"
        );
        // 2. aptu produces exactly 1 Calls edge per file
        assert_eq!(
            aptu_calls_a_count, 1,
            "aptu builder must produce 1 Calls edge for src/a.rs"
        );
        assert_eq!(
            aptu_calls_b_count, 1,
            "aptu builder must produce 1 Calls edge for src/b.rs"
        );
        // 3. Name-pair sets agree between both builders: {("main", "helper")}
        let expected_cross: HashSet<(String, String)> =
            [("main".to_string(), "helper".to_string())]
                .into_iter()
                .collect();
        assert_eq!(
            aptu_cross_calls, expected_cross,
            "aptu-core builder cross-file name-pair Calls edges"
        );
        assert_eq!(
            structural_cross_calls, expected_cross,
            "StructuralGraph cross-file name-pair Calls edges"
        );
        // 4. StructuralGraph edges do not cross files (same-file preference
        //    resolves main->helper within same file)
        assert!(
            !edges_cross_files,
            "StructuralGraph Calls edges must resolve within the same file"
        );
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
