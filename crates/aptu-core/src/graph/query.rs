// SPDX-License-Identifier: Apache-2.0

//! Graph queries: blast-radius BFS, ephemeral Modifies edges, and text rendering.
//!
//! The main entry point is [`blast_radius`], which performs a bounded BFS in both
//! directions from a set of modified nodes over [`Edge::Calls`], [`Edge::Implements`],
//! [`Edge::HasMethod`], and [`Edge::Tests`] edges, returning the induced subgraph.

use std::collections::{HashSet, VecDeque};

use petgraph::Direction;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef as _;

use super::{Edge, GraphDb, Node};

/// Adds ephemeral `Modifies` edges from the set of modified symbol names to their
/// nodes in the graph. Returns the node indices of all matched nodes.
///
/// A symbol matches if its `Node::name()` exactly equals one of `modified_symbols`.
/// The `Modifies` edges are not cached and must be stripped before serialisation.
#[must_use]
pub fn add_modifies_edges(graph: &mut GraphDb, modified_symbols: &[&str]) -> Vec<NodeIndex> {
    let symbol_set: HashSet<&str> = modified_symbols.iter().copied().collect();

    // Collect matching node indices (avoid borrow issues by collecting first).
    let matched: Vec<NodeIndex> = graph
        .node_indices()
        .filter(|&idx| {
            let name = graph[idx].name();
            symbol_set.contains(name)
        })
        .collect();

    // Add a sentinel source node that emits Modifies edges to each matched node.
    // This keeps the ephemeral edges structurally separate from static edges.
    // If there are no matched nodes, skip adding the sentinel.
    if !matched.is_empty() {
        let sentinel = graph.add_node(Node::File {
            name: "<diff>".to_string(),
            path: String::new(),
        });
        for &node in &matched {
            graph.add_edge(sentinel, node, Edge::Modifies);
        }
    }

    matched
}

/// Computes the blast-radius subgraph from a set of `modified_nodes`.
///
/// Performs a bounded BFS in both directions (callers and callees) from each
/// node in `modified_nodes` over [`Edge::Calls`], [`Edge::Implements`],
/// [`Edge::HasMethod`], and [`Edge::Tests`] edges. [`Edge::Contains`] and
/// [`Edge::Modifies`] are excluded.
///
/// The returned graph contains at most `max_nodes` nodes (including the seed
/// nodes). Traversal stops as soon as the cap is reached.
#[must_use]
pub fn blast_radius(graph: &GraphDb, modified_nodes: &[NodeIndex], max_nodes: usize) -> GraphDb {
    if modified_nodes.is_empty() || max_nodes == 0 {
        return GraphDb::new();
    }

    let relevant_edges = |e: &Edge| {
        matches!(
            e,
            Edge::Calls | Edge::Implements | Edge::HasMethod | Edge::Tests
        )
    };

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();

    for &node in modified_nodes {
        if graph.node_weight(node).is_some() && visited.insert(node) {
            queue.push_back(node);
        }
    }

    while let Some(current) = queue.pop_front() {
        if visited.len() >= max_nodes {
            break;
        }

        // Walk outgoing edges (callees).
        for edge_ref in graph.edges_directed(current, Direction::Outgoing) {
            if relevant_edges(edge_ref.weight()) {
                let target = edge_ref.target();
                if visited.len() < max_nodes && visited.insert(target) {
                    queue.push_back(target);
                }
            }
        }

        // Walk incoming edges (callers).
        for edge_ref in graph.edges_directed(current, Direction::Incoming) {
            if relevant_edges(edge_ref.weight()) {
                let source = edge_ref.source();
                if visited.len() < max_nodes && visited.insert(source) {
                    queue.push_back(source);
                }
            }
        }
    }

    // Build induced subgraph from visited nodes.
    build_induced_subgraph(graph, &visited)
}

/// Builds an induced subgraph containing only the nodes in `node_set` and
/// the edges between them (excluding [`Edge::Modifies`] and [`Edge::Contains`]).
fn build_induced_subgraph(graph: &GraphDb, node_set: &HashSet<NodeIndex>) -> GraphDb {
    use std::collections::HashMap;
    let mut sub = GraphDb::new();
    let mut index_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();

    // Add nodes.
    for &old_idx in node_set {
        if let Some(weight) = graph.node_weight(old_idx) {
            let new_idx = sub.add_node(weight.clone());
            index_map.insert(old_idx, new_idx);
        }
    }

    // Add edges between nodes in the subgraph.
    for edge_ref in graph.edge_references() {
        let src = edge_ref.source();
        let dst = edge_ref.target();
        let weight = edge_ref.weight();
        if matches!(weight, Edge::Modifies | Edge::Contains) {
            continue;
        }
        if let (Some(&new_src), Some(&new_dst)) = (index_map.get(&src), index_map.get(&dst)) {
            sub.add_edge(new_src, new_dst, *weight);
        }
    }

    sub
}

/// Renders a structural subgraph as compact human-readable text for prompt injection.
///
/// Each node is rendered as one line in the format:
/// ```text
/// fn foo [calls: bar, baz] [callers: qux]
/// ```
///
/// Only `Function` nodes are listed by default; `Struct`, `Enum`, `Trait`, and
/// `Impl` nodes appear without call/caller annotations. `File` and `Module`
/// nodes are omitted (they are structural, not behavioural).
#[must_use]
pub fn render_subgraph_text(subgraph: &GraphDb) -> String {
    use std::collections::HashMap;
    let mut lines: Vec<String> = Vec::new();

    // Build adjacency maps for calls and callers.
    let mut calls: HashMap<NodeIndex, Vec<String>> = HashMap::new();
    let mut callers: HashMap<NodeIndex, Vec<String>> = HashMap::new();

    for edge_ref in subgraph.edge_references() {
        if matches!(edge_ref.weight(), Edge::Calls) {
            calls
                .entry(edge_ref.source())
                .or_default()
                .push(subgraph[edge_ref.target()].name().to_string());
            callers
                .entry(edge_ref.target())
                .or_default()
                .push(subgraph[edge_ref.source()].name().to_string());
        }
        if matches!(edge_ref.weight(), Edge::HasMethod | Edge::Implements) {
            calls
                .entry(edge_ref.source())
                .or_default()
                .push(subgraph[edge_ref.target()].name().to_string());
        }
    }

    for idx in subgraph.node_indices() {
        let node = &subgraph[idx];
        let prefix = match node {
            Node::Function { .. } => "fn",
            Node::Struct { .. } => "struct",
            Node::Enum { .. } => "enum",
            Node::Trait { .. } => "trait",
            Node::Impl { .. } => "impl",
            Node::File { .. } | Node::Module { .. } => continue,
        };
        let name = node.name();
        let mut parts = vec![format!("{prefix} {name}")];
        if let Some(c) = calls.get(&idx) {
            parts.push(format!("[calls: {}]", c.join(", ")));
        }
        if let Some(c) = callers.get(&idx) {
            parts.push(format!("[callers: {}]", c.join(", ")));
        }
        lines.push(parts.join(" "));
    }

    lines.sort();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_caller_graph() -> (GraphDb, NodeIndex, NodeIndex, NodeIndex) {
        let mut graph = GraphDb::new();
        let target = graph.add_node(Node::Function {
            name: "target".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "pub".to_string(),
        });
        let caller_a = graph.add_node(Node::Function {
            name: "caller_a".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "private".to_string(),
        });
        let caller_b = graph.add_node(Node::Function {
            name: "caller_b".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "private".to_string(),
        });
        graph.add_edge(caller_a, target, Edge::Calls);
        graph.add_edge(caller_b, target, Edge::Calls);
        (graph, target, caller_a, caller_b)
    }

    #[test]
    fn test_blast_radius_returns_all_direct_callers_of_modified_node() {
        // Arrange: target has two callers.
        let (graph, target, caller_a, caller_b) = two_caller_graph();

        // Act
        let sub = blast_radius(&graph, &[target], 100);

        // Assert: all three nodes are in the subgraph.
        let names: Vec<&str> = sub.node_weights().map(|n| n.name()).collect();
        assert!(names.contains(&"target"), "target must be in subgraph");
        assert!(names.contains(&"caller_a"), "caller_a must be in subgraph");
        assert!(names.contains(&"caller_b"), "caller_b must be in subgraph");
    }

    #[test]
    fn test_blast_radius_bounded_by_max_nodes() {
        // Arrange: a chain of 10 nodes.
        let mut graph = GraphDb::new();
        let mut prev = graph.add_node(Node::Function {
            name: "n0".to_string(),
            path: "".to_string(),
            visibility: "pub".to_string(),
        });
        let root = prev;
        for i in 1..10usize {
            let next = graph.add_node(Node::Function {
                name: format!("n{i}"),
                path: "".to_string(),
                visibility: "pub".to_string(),
            });
            graph.add_edge(prev, next, Edge::Calls);
            prev = next;
        }

        // Act: cap at 3 nodes.
        let sub = blast_radius(&graph, &[root], 3);

        // Assert: at most 3 nodes in the subgraph.
        assert!(
            sub.node_count() <= 3,
            "blast_radius must respect max_nodes cap; got {}",
            sub.node_count()
        );
    }

    #[test]
    fn test_blast_radius_empty_when_max_nodes_zero() {
        let (graph, target, _, _) = two_caller_graph();
        let sub = blast_radius(&graph, &[target], 0);
        assert_eq!(sub.node_count(), 0);
    }

    #[test]
    fn test_render_subgraph_text_contains_function_with_caller() {
        // Arrange: two-caller graph.
        let (graph, target, _caller_a, _caller_b) = two_caller_graph();
        let sub = blast_radius(&graph, &[target], 100);

        // Act
        let text = render_subgraph_text(&sub);

        // Assert: rendered text contains the target function.
        assert!(text.contains("fn target"), "must render target function");
        // Must contain at least one caller annotation.
        assert!(text.contains("[callers:"), "must render callers annotation");
    }

    #[test]
    fn test_add_modifies_edges_matches_named_nodes() {
        // Arrange
        let mut graph = GraphDb::new();
        graph.add_node(Node::Function {
            name: "foo".to_string(),
            path: "".to_string(),
            visibility: "pub".to_string(),
        });
        graph.add_node(Node::Function {
            name: "bar".to_string(),
            path: "".to_string(),
            visibility: "pub".to_string(),
        });

        // Act
        let matched = add_modifies_edges(&mut graph, &["foo"]);

        // Assert: exactly one node matched.
        assert_eq!(matched.len(), 1);
        // A Modifies edge must be present.
        let has_modifies = graph
            .edge_indices()
            .any(|e| matches!(graph.edge_weight(e), Some(Edge::Modifies)));
        assert!(has_modifies, "Modifies edge must be added for matched node");
    }
}
