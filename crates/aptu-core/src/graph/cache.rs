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

use std::path::PathBuf;

use super::{Edge, GraphDb};
use crate::config::GraphConfig;

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
#[must_use]
pub fn strip_modifies_edges(graph: &GraphDb) -> GraphDb {
    let mut filtered = graph.clone();
    filtered.retain_edges(|g, edge_idx| !matches!(g.edge_weight(edge_idx), Some(Edge::Modifies)));
    filtered
}

/// Encodes `graph` into the on-disk cache byte format.
///
/// Strips `Modifies` edges, then prepends the 4-byte `FORMAT_VERSION` header
/// to the bincode-encoded payload. Returns `None` if bincode serialization fails.
#[must_use]
pub fn encode_graph(graph: &GraphDb) -> Option<Vec<u8>> {
    let stripped = strip_modifies_edges(graph);
    let payload = bincode::serde::encode_to_vec(&stripped, bincode::config::standard()).ok()?;

    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&payload);
    Some(bytes)
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
        bincode::serde::decode_from_slice(&bytes[4..], bincode::config::standard()).ok()?;
    Some(graph)
}

/// Loads a cached graph from disk, or builds and caches a new one from the
/// rendered context strings.
///
/// Returns `(graph, cache_hit)`. On any cache read failure (missing file,
/// version mismatch, decode error), falls through to building a new graph.
///
/// On WASM targets, this always builds a new graph (no disk I/O).
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn load_or_build(
    owner: &str,
    repo: &str,
    sha: &str,
    ast: &str,
    call_graph: &str,
    cfg: &GraphConfig,
) -> (GraphDb, bool) {
    // Try cache first.
    let path = cache_path(owner, repo, sha);
    if let Ok(Some(cached)) = try_load_cached(&path, cfg) {
        return (cached, true);
    }

    // Build from scratch.
    let mut graph = super::builder::parse_ast_context_string(ast);
    super::builder::parse_call_graph_string(call_graph, &mut graph);

    // Persist to cache.
    persist_graph(&path, &graph);

    (graph, false)
}

/// WASM fallback: always build from scratch, no disk I/O.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn load_or_build(
    _owner: &str,
    _repo: &str,
    _sha: &str,
    ast: &str,
    call_graph: &str,
    _cfg: &GraphConfig,
) -> (GraphDb, bool) {
    let mut graph = super::builder::parse_ast_context_string(ast);
    super::builder::parse_call_graph_string(call_graph, &mut graph);
    (graph, false)
}

/// Tries to load a cached graph from `path`.
///
/// Returns `None` if the file doesn't exist, is too old (TTL expired),
/// or fails to decode.
#[cfg(not(target_arch = "wasm32"))]
fn try_load_cached(path: &PathBuf, cfg: &GraphConfig) -> std::io::Result<Option<GraphDb>> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };

    // Check TTL.
    let modified = metadata
        .modified()
        .unwrap_or_else(|_| std::time::SystemTime::now());
    let age = std::time::SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default();
    let ttl = std::time::Duration::from_secs(cfg.cache_ttl_hours * 3600);
    if age > ttl {
        // Expired; caller will rebuild.
        return Ok(None);
    }

    let bytes = std::fs::read(path)?;
    Ok(decode_graph(&bytes))
}

/// Persists a graph to the cache path.
///
/// Creates parent directories if needed. Failures are logged at WARN level
/// and never propagated (caching is best-effort).
#[cfg(not(target_arch = "wasm32"))]
fn persist_graph(path: &PathBuf, graph: &GraphDb) {
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(
            path = %parent.display(),
            error = %e,
            "graph cache: failed to create cache directory"
        );
        return;
    }

    let Some(bytes) = encode_graph(graph) else {
        tracing::warn!(path = %path.display(), "graph cache: encode failed, skipping write");
        return;
    };

    // Atomic write: write to a sibling temp file then rename to prevent partial
    // file corruption if the process crashes during the write.
    let tmp_path = path.with_extension("tmp");
    if let Err(e) = std::fs::write(&tmp_path, &bytes) {
        tracing::warn!(
            path = %tmp_path.display(),
            error = %e,
            "graph cache: failed to write temp file"
        );
        return;
    }
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        tracing::warn!(
            src = %tmp_path.display(),
            dst = %path.display(),
            error = %e,
            "graph cache: failed to rename temp file to cache path"
        );
        // Best-effort cleanup; ignore error.
        let _ = std::fs::remove_file(&tmp_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_round_trip_serialize_deserialize() {
        let mut graph = GraphDb::new();
        let n1 = graph.add_node(super::super::Node::Function {
            name: "foo".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "pub".to_string(),
        });
        let n2 = graph.add_node(super::super::Node::Function {
            name: "bar".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "pub".to_string(),
        });
        graph.add_edge(n1, n2, Edge::Calls);

        let bytes = encode_graph(&graph).expect("encode must succeed");
        let decoded = decode_graph(&bytes).expect("should decode successfully");

        assert_eq!(
            graph.node_count(),
            decoded.node_count(),
            "node count should match"
        );
        assert_eq!(
            graph.edge_count(),
            decoded.edge_count(),
            "edge count should match"
        );

        // Verify node names survived round-trip.
        let names: Vec<String> = decoded
            .node_indices()
            .map(|idx| decoded[idx].name().to_string())
            .collect();
        assert!(names.contains(&"foo".to_string()));
        assert!(names.contains(&"bar".to_string()));
    }

    #[test]
    fn test_decode_graph_version_mismatch() {
        let mut graph = GraphDb::new();
        graph.add_node(super::super::Node::Function {
            name: "foo".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "pub".to_string(),
        });

        // Encode, then corrupt the version byte.
        let mut bytes = encode_graph(&graph).expect("encode must succeed");
        bytes[0] = 0xFF; // Wrong version.

        let result = decode_graph(&bytes);
        assert!(result.is_none(), "version mismatch should return None");
    }

    #[test]
    fn test_decode_graph_empty_bytes() {
        let result = decode_graph(&[]);
        assert!(result.is_none(), "empty bytes should return None");
    }

    #[test]
    fn test_strip_modifies_edges() {
        let mut graph = GraphDb::new();
        let n1 = graph.add_node(super::super::Node::Function {
            name: "foo".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "pub".to_string(),
        });
        let n2 = graph.add_node(super::super::Node::Function {
            name: "bar".to_string(),
            path: "src/lib.rs".to_string(),
            visibility: "pub".to_string(),
        });
        graph.add_edge(n1, n2, Edge::Calls);
        graph.add_edge(n1, n2, Edge::Modifies);

        let stripped = strip_modifies_edges(&graph);
        // Only the Calls edge should remain.
        let has_modifies = stripped
            .edge_indices()
            .any(|idx| matches!(stripped.edge_weight(idx), Some(Edge::Modifies)));
        assert!(!has_modifies, "Modifies edges should be stripped");
        assert_eq!(stripped.edge_count(), 1, "only Calls edge should remain");
    }

    #[test]
    fn test_cache_path_format() {
        let path = cache_path("owner", "repo", "abc123");
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("owner"), "path should contain owner");
        assert!(path_str.contains("repo"), "path should contain repo");
        assert!(path_str.contains("abc123"), "path should contain sha");
        assert!(path_str.ends_with(".bin"), "path should end with .bin");
    }
}
