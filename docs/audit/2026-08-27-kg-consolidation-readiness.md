# KG Consolidation Readiness Audit

> **Status: Superseded.** Graph consolidation into `StructuralGraph` was implemented across PRs #1544, #1551, #1553, and #1554. See `docs/ARCHITECTURE.md` for current structural graph architecture.

Date: 2026-08-27  
Related: #1510, #1525, #1528, #1532, #1533  
Blocks: clouatre-labs/aptu-coder#1472

## Context

PR #1532 bumped aptu-coder-core to 0.32.0 and verified Calls edge parity between aptu's `graph::builder` and `StructuralGraph::build_from_analysis`. Issue #1528 is resolved. The next logical step is consolidating aptu's duplicate graph module (`crates/aptu-core/src/graph/`) into `StructuralGraph` from aptu-coder-core.

## Current State

aptu's graph module (`graph/`) provides:

- `builder.rs`: `build_from_analysis()` - builds `GraphDb` from `SemanticAnalysis` + `CallGraph`
- `query.rs`: `blast_radius()` (bidirectional BFS), `render_subgraph_text()` (prompt-ready text), `find_modified_nodes()`
- `cache.rs`: on-disk cache keyed by `(owner, repo, sha)`, WASM-safe, TTL-based expiration
- `mod.rs`: `Node` (File/Module/Function), `Edge` (Contains/Calls/Imports), `GraphDb` type alias

aptu-coder-core's `StructuralGraph` provides:

- `build_from_analysis()` / `from_call_graph()` - builds from `&[FileAnalysisOutput]`
- `bfs_blast_radius()` / `blast_radius_subgraph()` - outgoing-only BFS
- `GraphDiskStore` - sharded LRU cache with file locking
- Cross-file disambiguation (same-file preference, line proximity, arg-count fallback) - new in 0.32.0

## Gap Analysis

### F1: No text rendering (Critical)

aptu's `render_subgraph_text()` produces prompt-ready text grouped by file path, showing `fn name [calls: a, b] [callers: c]` per function. `StructuralGraph` has no text rendering method. This is the core PR review feature; without it, aptu cannot drop its query module.

### F2: Unidirectional BFS (Critical)

aptu's `blast_radius()` walks both `Direction::Incoming` (callers) and `Direction::Outgoing` (callees), filtered to `Edge::Calls`. `StructuralGraph::bfs_frontier()` uses `graph.neighbors()` which is outgoing-only. Blast-radius must include callers to be useful for impact analysis.

### F3: No max_nodes cap (Critical)

aptu caps the blast-radius subgraph at `max_nodes` (default 50,000) to prevent prompt explosion on large repositories. `StructuralGraph`'s BFS has no node cap.

### F4: Single-seed only (Critical)

aptu accepts `&[NodeIndex]` (multiple modified functions per PR). `StructuralGraph::bfs_blast_radius()` and `blast_radius_subgraph()` accept a single `symbol: &str`. PRs touch multiple functions.

### F5: GraphDiskStore not WASM-safe (Critical)

`GraphDiskStore` uses `fs2` (file locking) and `NamedTempFile`, neither of which compile under `wasm32-unknown-unknown`. aptu's `cache.rs` gates all I/O behind `#[cfg(not(target_arch = "wasm32"))]`. aptu-coder-core must do the same to preserve aptu's WASM target.

## Recommendations

### R1: Implement in aptu-coder-core (issue #1472)

Add to `StructuralGraph`:

1. `render_subgraph_text(&self, nodes: &[NodeIndex]) -> String` matching aptu's output format
2. `blast_radius_bidirectional(&self, seeds: &[NodeIndex], max_nodes: usize, max_depth: usize) -> (Vec<NodeIndex>, Vec<(NodeIndex, NodeIndex, Edge)>)` walking both directions over `Edge::Calls` only
3. Gate `GraphDiskStore` behind `#[cfg(not(target_arch = "wasm32"))]`

### R2: Consolidate in aptu (issue #1533)

After aptu-coder ships a release with R1:

1. Replace `graph::builder` with `StructuralGraph::build_from_analysis` / `from_call_graph`
2. Replace `graph::query` with StructuralGraph's new methods
3. Replace `graph::cache` with `GraphDiskStore` (or thin adapter)
4. Remove `graph::Node`, `graph::Edge`, `graph::GraphDb`
5. Update `ast_context.rs`, `review_context.rs`, `config/graph.rs`

### R3: Release gate

No `[patch]` or git-dependency workarounds. The crates.io release is the gate, per #1528's constraint.

## Conclusion

Five critical gaps remain. All are in aptu-coder-core. The consolidation is a two-PR, two-release sequence: aptu-coder first (issue #1472), then aptu after a crates.io release ships (issue #1533).
