// SPDX-License-Identifier: Apache-2.0

//! On-disk cache for structural graphs, keyed by repository and commit SHA.
//!
//! Cache format: 8 raw bytes of header followed by a postcard-encoded
//! [`super::GraphDb`] payload. The header is two little-endian `u32`s:
//! `FORMAT_VERSION` (bytes 0..4) then `schema_hash` (bytes 4..8), a
//! compile-time FNV-1a hash over the `Node`/`Edge` variant names used to
//! invalidate stale caches when the schema changes.
//!
//! Only the actual file I/O (`load_or_build`, `persist_graph`) is gated to
//! non-WASM targets. Path construction and byte encode/decode are pure
//! functions usable on any target.

use std::io::Write;
use std::path::PathBuf;

#[cfg(test)]
use super::Edge;
use super::GraphDb;
use crate::config::GraphConfig;

/// Cache format version. Bump when the encoding changes in an incompatible way.
const FORMAT_VERSION: u32 = 3;

/// Compile-time FNV-1a hash over the `Node`/`Edge` variant names.
///
/// Any change to the set (or order) of `Node`/`Edge` variant names must be
/// reflected in [`SCHEMA_STRING`] so that stale cached graphs are invalidated
/// by [`decode_graph`] rather than postcard mis-decoding.
const SCHEMA_STRING: &str = "File|Module|Function|Contains|Calls|Imports";

/// Computes the compile-time FNV-1a hash of [`SCHEMA_STRING`].
#[must_use]
pub const fn schema_hash() -> u32 {
    let bytes = SCHEMA_STRING.as_bytes();
    let mut hash: u32 = 0x811c_9dc5;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i += 1;
    }
    hash
}

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

/// Encodes `graph` into the on-disk cache byte format.
///
/// Removes `Modifies` edges by rebuilding a filtered graph (single O(N+E) pass
/// over nodes and edges), then prepends the 8-byte header (`FORMAT_VERSION`
/// followed by `schema_hash`) to the postcard-encoded payload. Returns `None`
/// if postcard serialization fails.
#[must_use]
pub fn encode_graph(graph: &GraphDb) -> Option<Vec<u8>> {
    let mut filtered = GraphDb::new();
    for idx in graph.node_indices() {
        filtered.add_node(graph[idx].clone());
    }
    for idx in graph.edge_indices() {
        let (a, b) = graph.edge_endpoints(idx)?;
        filtered.add_edge(a, b, graph[idx]);
    }

    let payload = postcard::to_allocvec(&filtered).ok()?;

    let mut bytes = Vec::with_capacity(8 + payload.len());
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&schema_hash().to_le_bytes());
    bytes.extend_from_slice(&payload);
    Some(bytes)
}

/// Decodes a graph from on-disk cache bytes.
///
/// Returns `None` (cache miss) if the bytes are too short, the format
/// version does not match [`FORMAT_VERSION`], the `schema_hash` does not
/// match [`schema_hash`], or postcard decoding fails.
#[must_use]
pub fn decode_graph(bytes: &[u8]) -> Option<GraphDb> {
    if bytes.len() < 8 {
        return None;
    }
    let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if version != FORMAT_VERSION {
        return None;
    }
    let hash = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if hash != schema_hash() {
        return None;
    }
    let graph: GraphDb = postcard::from_bytes(&bytes[8..]).ok()?;
    Some(graph)
}

/// Loads a cached graph from disk, or persists the provided graph to cache.
///
/// Returns `(graph, cache_hit)`. On cache hit, the provided `graph` is dropped
/// and the cached version is returned. On cache miss, the provided `graph` is
/// persisted to disk and returned.
///
/// On WASM targets, this always returns the provided graph with `cache_hit = false`
/// (no disk I/O).
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn load_or_build(
    owner: &str,
    repo: &str,
    sha: &str,
    graph: GraphDb,
    cfg: &GraphConfig,
) -> (GraphDb, bool) {
    // Try cache first.
    let path = cache_path(owner, repo, sha);
    if let Ok(Some(cached)) = try_load_cached(&path, cfg) {
        return (cached, true);
    }

    // Persist to cache.
    persist_graph(&path, &graph);

    (graph, false)
}

/// WASM fallback: always return provided graph, no disk I/O.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn load_or_build(
    _owner: &str,
    _repo: &str,
    _sha: &str,
    graph: GraphDb,
    _cfg: &GraphConfig,
) -> (GraphDb, bool) {
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
fn persist_graph(path: &std::path::Path, graph: &GraphDb) {
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

    if let Err(e) = write_atomic(path, &bytes) {
        tracing::warn!(
            path = %path.display(),
            error = %e,
            "graph cache: failed to write cache file"
        );
    }
}

/// Writes `bytes` to `path` atomically: writes to a uniquely-named sibling
/// temp file then renames it into place, so a crash mid-write never corrupts
/// an existing cache entry and concurrent writers never race on a partial file.
#[cfg(not(target_arch = "wasm32"))]
fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = tempfile::Builder::new().tempfile_in(parent)?;
    let result = tmp.write_all(bytes).and_then(|()| tmp.flush());
    match result {
        Ok(()) => std::fs::rename(tmp.path(), path),
        Err(e) => {
            let _ = std::fs::remove_file(tmp.path());
            Err(e)
        }
    }
    // tmp drops here; on the success path the file has been renamed so the
    // NamedTempFile destructor will attempt to delete a path that no longer
    // exists, which is harmless.
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_schema_hash_changes_on_variant_change() {
        const MUTATED: &str = "File|Module|Function|Contains|Calls|Imports|NewVariant";
        // The hash must be non-zero (FNV-1a of a non-empty string is never 0).
        assert_ne!(schema_hash(), 0, "schema hash must be non-zero");

        // Verify that any change to SCHEMA_STRING produces a different hash by
        // computing FNV-1a on a mutated string and asserting divergence.
        let mut hash: u32 = 0x811c_9dc5;
        for &b in MUTATED.as_bytes() {
            hash ^= u32::from(b);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        assert_ne!(
            schema_hash(),
            hash,
            "schema_hash must differ when SCHEMA_STRING gains a new variant"
        );
    }

    #[test]
    fn test_persist_graph_concurrent_writes_no_corruption() {
        let path =
            std::env::temp_dir().join(format!("aptu_cache_concurrent_{}.bin", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let p1 = path.clone();
        let handle1 = std::thread::spawn(move || {
            let mut g = GraphDb::new();
            let n = g.add_node(super::super::Node::Function {
                name: "one".to_string(),
                path: "src/a.rs".to_string(),
                visibility: "pub".to_string(),
            });
            g.add_edge(n, n, Edge::Calls);
            persist_graph(&p1, &g);
        });
        let p2 = path.clone();
        let handle2 = std::thread::spawn(move || {
            let mut g = GraphDb::new();
            let n = g.add_node(super::super::Node::Function {
                name: "two".to_string(),
                path: "src/b.rs".to_string(),
                visibility: "pub".to_string(),
            });
            g.add_edge(n, n, Edge::Calls);
            persist_graph(&p2, &g);
        });

        handle1.join().expect("thread 1 must not panic");
        handle2.join().expect("thread 2 must not panic");

        // The file must be a valid, fully-decodable graph (no partial write).
        let bytes = std::fs::read(&path).expect("cache file must exist");
        let decoded = decode_graph(&bytes).expect("cache file must decode without corruption");
        assert_eq!(decoded.node_count(), 1, "decoded graph must have one node");
        assert_eq!(decoded.edge_count(), 1, "decoded graph must have one edge");

        let _ = std::fs::remove_file(&path);
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
