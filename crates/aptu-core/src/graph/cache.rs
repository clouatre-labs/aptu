// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2025 Agentic AI Foundation

//! On-disk cache for structural graphs, keyed by repository and commit SHA.

use std::io::Write;
use std::path::PathBuf;

use super::StructuralGraph;
use crate::config::GraphConfig;

const FORMAT_VERSION: u32 = 4;
const SCHEMA_STRING: &str = "StructuralGraph|File|Symbol|Module|Contains|Calls|Imports";

/// Computes the cache schema hash.
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

#[allow(missing_docs)]
/// Returns the cache file path for a repository revision.
#[must_use]
pub fn cache_path(owner: &str, repo: &str, sha: &str) -> PathBuf {
    crate::config::data_dir()
        .join("graph")
        .join(owner)
        .join(repo)
        .join(format!("{sha}.bin"))
}

#[allow(missing_docs)]
#[must_use]
pub fn encode_graph(graph: &StructuralGraph) -> Option<Vec<u8>> {
    let payload = postcard::to_allocvec(graph).ok()?;
    let mut bytes = Vec::with_capacity(8 + payload.len());
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&schema_hash().to_le_bytes());
    bytes.extend_from_slice(&payload);
    Some(bytes)
}

/// Decodes a `StructuralGraph` from a versioned, schema-checked cache payload.
#[must_use]
pub fn decode_graph(bytes: &[u8]) -> Option<StructuralGraph> {
    if bytes.len() < 8
        || u32::from_le_bytes(bytes[0..4].try_into().ok()?) != FORMAT_VERSION
        || u32::from_le_bytes(bytes[4..8].try_into().ok()?) != schema_hash()
    {
        return None;
    }
    postcard::from_bytes(&bytes[8..]).ok()
}

/// Loads a cached graph or persists and returns the supplied graph.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn load_or_build(
    owner: &str,
    repo: &str,
    sha: &str,
    graph: StructuralGraph,
    cfg: &GraphConfig,
) -> (StructuralGraph, bool) {
    let path = cache_path(owner, repo, sha);
    if let Ok(Some(cached)) = try_load_cached(&path, cfg) {
        return (cached, true);
    }
    persist_graph(&path, &graph);
    (graph, false)
}

#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn load_or_build(
    _: &str,
    _: &str,
    _: &str,
    graph: StructuralGraph,
    _: &GraphConfig,
) -> (StructuralGraph, bool) {
    (graph, false)
}

#[cfg(not(target_arch = "wasm32"))]
fn try_load_cached(path: &PathBuf, cfg: &GraphConfig) -> std::io::Result<Option<StructuralGraph>> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let age = std::time::SystemTime::now()
        .duration_since(
            metadata
                .modified()
                .unwrap_or_else(|_| std::time::SystemTime::now()),
        )
        .unwrap_or_default();
    if age > std::time::Duration::from_secs(cfg.cache_ttl_hours * 3600) {
        return Ok(None);
    }
    Ok(decode_graph(&std::fs::read(path)?))
}

#[cfg(not(target_arch = "wasm32"))]
fn persist_graph(path: &std::path::Path, graph: &StructuralGraph) {
    let Some(parent) = path.parent() else { return };
    if let Err(e) = std::fs::create_dir_all(parent) {
        tracing::warn!(error = %e, "graph cache directory creation failed");
        return;
    }
    let Some(bytes) = encode_graph(graph) else {
        return;
    };
    if let Err(e) = write_atomic(path, &bytes) {
        tracing::warn!(error = %e, "graph cache write failed");
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = tempfile::Builder::new().tempfile_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.flush()?;
    std::fs::rename(tmp.path(), path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aptu_coder_core::types::{FunctionInfo, SemanticAnalysis};

    fn graph(path: &str, name: &str) -> StructuralGraph {
        let mut function = FunctionInfo::default();
        function.name = name.to_string();
        function.line = 1;
        function.end_line = 2;
        let semantic = SemanticAnalysis::new(
            vec![function],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            std::collections::HashMap::default(),
            Vec::new(),
            Vec::new(),
        );
        StructuralGraph::build_from_analysis(&[aptu_coder_core::FileAnalysisOutput::new(
            path.to_string(),
            String::new(),
            semantic,
            2,
            None,
        )])
    }

    #[test]
    fn structural_graph_payload_round_trips() {
        let original = graph("src/lib.rs", "round_trip");
        let decoded = decode_graph(&encode_graph(&original).unwrap()).unwrap();
        assert_eq!(original.graph.node_count(), decoded.graph.node_count());
        assert_eq!(original.graph.edge_count(), decoded.graph.edge_count());
    }

    #[test]
    fn old_version_and_schema_hash_are_rejected() {
        let mut bytes = encode_graph(&graph("src/lib.rs", "versioned")).unwrap();
        bytes[..4].copy_from_slice(&3u32.to_le_bytes());
        assert!(decode_graph(&bytes).is_none());
        let mut bytes = encode_graph(&graph("src/lib.rs", "hashed")).unwrap();
        bytes[4..8].copy_from_slice(&(schema_hash() ^ 1).to_le_bytes());
        assert!(decode_graph(&bytes).is_none());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn load_or_build_isolated_by_revision_and_honors_ttl() {
        let owner = format!("aptu-test-{}", uuid_suffix());
        let repo = "graph-cache";
        let sha = uuid_suffix();
        let cfg = GraphConfig {
            cache_ttl_hours: 24,
            ..GraphConfig::default()
        };
        let (_, reused) = load_or_build(&owner, repo, &sha, graph("a", "first"), &cfg);
        assert!(!reused);
        let (cached, reused) = load_or_build(&owner, repo, &sha, graph("a", "second"), &cfg);
        assert!(reused);
        assert!(cached.graph.node_indices().any(|i| matches!(&cached.graph[i], aptu_coder_core::graph::Node::Symbol { name, .. } if name == "first")));
        let (_, different_revision) =
            load_or_build(&owner, repo, "other-sha", graph("b", "other"), &cfg);
        assert!(!different_revision);
        let expired = GraphConfig {
            cache_ttl_hours: 0,
            ..cfg
        };
        let (_, reused) = load_or_build(&owner, repo, &sha, graph("a", "expired"), &expired);
        assert!(!reused);
        let _ = std::fs::remove_dir_all(crate::config::data_dir().join("graph").join(owner));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn concurrent_atomic_writes_remain_decodable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.bin");
        let p1 = path.clone();
        let p2 = path.clone();
        let a = std::thread::spawn(move || persist_graph(&p1, &graph("a", "one")));
        let b = std::thread::spawn(move || persist_graph(&p2, &graph("b", "two")));
        a.join().unwrap();
        b.join().unwrap();
        assert!(decode_graph(&std::fs::read(path).unwrap()).is_some());
    }

    fn uuid_suffix() -> String {
        format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }
}
