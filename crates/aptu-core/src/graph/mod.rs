// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2025 Agentic AI Foundation

//! Structural graph adapter backed by `aptu-coder-core`.

pub mod cache;
pub use aptu_coder_core::graph::StructuralGraph;

/// Render the bounded bidirectional blast radius for modified symbol names.
#[must_use]
pub fn render_blast_radius(
    graph: &StructuralGraph,
    names: &[&str],
    max_nodes: usize,
    max_depth: usize,
) -> String {
    let seeds = graph.find_symbols(names);
    let (nodes, _) = graph.blast_radius_bidirectional(&seeds, max_nodes, max_depth);
    graph.render_subgraph_text(&nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> aptu_coder_core::FileAnalysisOutput {
        aptu_coder_core::analyze_file(path, None).unwrap()
    }

    /// Upstream renders duplicate tuples once per distinct node, while the old
    /// local renderer deduplicated by name; this test records stable ordering.
    #[test]
    fn blast_radius_render_is_deterministic_for_duplicate_names() {
        let dir = tempfile::tempdir().unwrap();
        let z = dir.path().join("z.rs");
        let a = dir.path().join("a.rs");
        std::fs::write(&z, "fn shared() {}\nfn z_call() { shared(); }\n").unwrap();
        std::fs::write(&a, "fn shared() {}\nfn a_call() { shared(); }\n").unwrap();
        let graph = StructuralGraph::build_from_analysis(&[
            entry(z.to_str().unwrap()),
            entry(a.to_str().unwrap()),
        ]);
        let first = render_blast_radius(&graph, &["shared"], 20, 3);
        let second = render_blast_radius(&graph, &["shared"], 20, 3);
        assert_eq!(first, second);
        assert!(!first.is_empty());
        assert!(first.contains("fn shared"));
    }
}
