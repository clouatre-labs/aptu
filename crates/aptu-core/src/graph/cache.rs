// SPDX-License-Identifier: Apache-2.0

//! On-disk cache for structural graphs, keyed by repository and commit SHA.
//!
//! Cache format: 4 raw bytes for `FORMAT_VERSION` (little-endian `u32`)
//! followed by a bincode-encoded [`super::GraphDb`] payload. `Modifies` edges
//! are ephemeral (derived from the current diff) and are always stripped
//! before serialization; they never appear in a cached graph.
//!
//! Only the actual file I/O (`load_or_build`) is gated to non-WASM targets.
//! Path construction and byte encode/decode are pure functions usable on any
//! target.

use std::path::{Path, PathBuf};

use super::{Edge, GraphDb};

/// Cache format version. Bump when the encoding changes in an incompatible way.
const FORMAT_VERSION: u32 = 1;

/// Returns the on-disk cache path for a given repository and commit SHA.
///
/// Path shape: `~/.local/share/aptu/graph/<owner>/<repo>/<sha>.bin`.
#[must_use]
pub fn cache_path(owner: &str, repo: &str, sha: &str) -> PathBuf {
    crate::config::data_dir()
        .join("graph")
        .join(owner)
        .join(repo)
        .join(format!("{sha}.bin"))
}

/// Strips `Modifies` edges from `graph`, returning a new graph without them.
///
/// `Modifies` edges are ephemeral (derived from the PR diff at review time)
/// and must never be persisted to the cache.
fn strip_modifies_edges(graph: &GraphDb) -> GraphDb {
    let mut filtered = graph.clone();
    filtered.retain_edges(|g, edge_idx| !matches!(g.edge_weight(edge_idx), Some(Edge::Modifies)));
    filtered
}

/// Encodes `graph` into the on-disk cache byte format.
///
/// Strips `Modifies` edges, then prepends the 4-byte `FORMAT_VERSION` header
/// to the bincode-encoded payload.
#[must_use]
pub fn encode_graph(graph: &GraphDb) -> Vec<u8> {
    let stripped = strip_modifies_edges(graph);
    let payload = bincode_next::serde::encode_to_vec(&stripped, bincode_next::config::standard())
        .unwrap_or_default();

    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

/// Decodes a graph from on-disk cache bytes.
///
/// Returns `None` (cache miss) if the bytes are too short, the format
/// version does not match [`FORMAT_VERSION`], or bincode decoding fails.
#[must_use]
pub fn decode_graph(bytes: &[u8]) -> Option<GraphDb> {
    if bytes.len() < 4 {
        return None;
    }
    let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if version != FORMAT_VERSION {
        return None;
    }
    let (graph, _len): (GraphDb, usize) =
        bincode_next::serde::decode_from_slice(&bytes[4..], bincode_next::config::standard())
            .ok()?;
    Some(graph)
}

/// Loads a cached graph from disk, or builds and caches a new one from `files`.
///
/// Returns `(graph, cache_hit)`. On any cache read failure (missing file,
/// version mismatch, decode error), the stale file is removed (if present)
/// and a fresh graph is built and written back to the cache.
#[cfg(not(target_arch = "wasm32"))]
pub async fn load_or_build(
    owner: &str,
    repo: &str,
    sha: &str,
    files: &[(&Path, &str)],
) -> (GraphDb, bool) {
    let path = cache_path(owner, repo, sha);

    if let Ok(bytes) = tokio::fs::read(&path).await {
        if let Some(graph) = decode_graph(&bytes) {
            return (graph, true);
        }
        // Stale or corrupt cache entry; remove it before rebuilding.
        let _ = tokio::fs::remove_file(&path).await;
    }

    let graph = super::build_graph(files);

    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let bytes = encode_graph(&graph);
    if let Err(e) = tokio::fs::write(&path, &bytes).await {
        tracing::warn!(path = %path.display(), error = %e, "graph: failed to write cache file");
    }

    (graph, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;

    fn sample_graph() -> GraphDb {
        let mut graph = GraphDb::new();
        let a = graph.add_node(Node::Function {
            name: "foo".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "pub".to_string(),
        });
        let b = graph.add_node(Node::Function {
            name: "bar".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "private".to_string(),
        });
        graph.add_edge(a, b, Edge::Calls);
        graph.add_edge(a, b, Edge::Modifies);
        graph
    }

    #[test]
    fn test_encode_decode_round_trip_identical_node_and_edge_count() {
        // Arrange: graph with a Modifies edge that must be stripped, plus a Calls edge.
        let graph = sample_graph();

        // Act
        let bytes = encode_graph(&graph);
        let decoded =
            decode_graph(&bytes).expect("decode should succeed for freshly encoded bytes");

        // Assert: node count identical; edge count reflects Modifies stripped (1 Calls edge only).
        assert_eq!(decoded.node_count(), graph.node_count());
        assert_eq!(
            decoded.edge_count(),
            1,
            "Modifies edge must be stripped before caching"
        );
    }

    #[test]
    fn test_decode_graph_mismatched_format_version_is_cache_miss() {
        // Arrange: encode with the real format, then corrupt the version header.
        let graph = sample_graph();
        let mut bytes = encode_graph(&graph);
        bytes[0..4].copy_from_slice(&999_u32.to_le_bytes());

        // Act
        let result = decode_graph(&bytes);

        // Assert: mismatched version is treated as a cache miss.
        assert!(
            result.is_none(),
            "mismatched format version must be rejected"
        );
    }

    #[test]
    fn test_cache_path_shape() {
        let path = cache_path("owner", "repo", "abc123");
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("graph"));
        assert!(
            path_str.ends_with("owner/repo/abc123.bin")
                || path_str.ends_with("owner\\repo\\abc123.bin")
        );
    }
}
