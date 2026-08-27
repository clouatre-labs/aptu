// SPDX-License-Identifier: Apache-2.0

//! Structural call graph for PR review context.
//!
//! Builds a petgraph-backed directed graph of source-code symbols (functions,
//! modules) directly from the typed `SemanticAnalysis` and `CallGraph` structs
//! produced by `aptu-coder-core`, caches the result on disk keyed by
//! repository and commit SHA, and computes a bounded blast-radius subgraph
//! around modified symbols for prompt injection.
//!
//! ## Why this duplicates `aptu_coder_core::graph::StructuralGraph`
//!
//! `aptu-coder-core` (a crates.io dependency, not a path/git dependency)
//! ships its own persisted structural graph, `StructuralGraph`. This module
//! does not build on it, and that is a deliberate decision, not an
//! oversight:
//!
//! - **Different input.** This module builds `GraphDb` from `CallGraph` (a
//!   `HashMap`-based, request-scoped representation with symbol-matching
//!   modes) plus raw `SemanticAnalysis`. `StructuralGraph` is built from
//!   `&[FileAnalysisOutput]`, a different, higher-level input path.
//! - **Actual ontology vs. stated.** This module's `build_from_analysis`
//!   function (the sole production builder) emits exactly `Node::File`,
//!   `Node::Function`, `Node::Module` and `Edge::Contains`, `Edge::Imports`,
//!   `Edge::Calls` — a strict subset of `StructuralGraph`'s model. The doc
//!   comment in issue #1512 claimed this module emits `Struct`/`Enum`/`Trait`
//!   /`Impl` nodes and `Implements`/`HasMethod`/`Tests` edges; that was
//!   accurate before PR #1445 (which replaced a text-format builder with the
//!   typed-struct builder) but not against the current code, and those dead
//!   variants were removed in #1521. See issue #1520 for the correction.
//! - **Input-shape gaps: one closed, one verified.** `StructuralGraph`
//!   previously derived file path from `entry.formatted.lines().next()`
//!   rather than a struct field; `aptu-coder-core` v0.31.0 (aptu-coder#1459)
//!   added an explicit `path: String` field to `FileAnalysisOutput`, and
//!   `StructuralGraph::build_from_analysis` now reads `entry.path` directly,
//!   closing that gap. The second gap — whether aptu's call-edge filtering
//!   (skipping `<reference>` pseudo-edges and `is_impl_trait` edges,
//!   `builder.rs:88-96`) has parity with `StructuralGraph`'s
//!   `SemanticAnalysis.calls`-based edges — is now verified rather than
//!   assumed: `StructuralGraph::build_from_analysis` builds `Calls` edges
//!   solely from `entry.semantic.calls` (`CallInfo`, real call sites) and
//!   never reads `SemanticAnalysis.references` or `impl_traits`. Those two
//!   fields are exactly where `CallGraph::build_from_results` (this crate's
//!   own `CallGraph` builder) synthesizes the `<reference>` pseudo-edges and
//!   impl-trait edges that `builder.rs` must filter out. The two builders
//!   agree by construction, not by matching filter logic: `StructuralGraph`
//!   never ingests the synthetic edge kinds aptu filters.
//!   Starting with `aptu-coder-core` 0.32.0, `StructuralGraph::build_from_analysis`
//!   resolves cross-file same-name collisions via same-file preference / line-proximity
//!   / arg-count fallback, while aptu's `build_from_analysis` operates per-file
//!   and is name-only. Name-pair output agrees; node-level resolution differs.
//!   `builder::tests::test_calls_parity_with_structural_graph` exercises
//!   both single-file and cross-file collision fixtures and asserts parity.
//!   Per #1510's acceptance criteria that `blast_radius()` output must not regress,
//!   this closes the second blocker too.
//! - **Precedent.** The same audit's F7/R6 already concluded that
//!   `StructuralGraph` and `CallGraph` should stay separate within
//!   `aptu-coder-core` because they serve different workloads; that
//!   reasoning extends here.
//! - **Release topology.** Extending `StructuralGraph` to cover this
//!   module's ontology would require a change in the `aptu-coder` repo, a
//!   new crates.io release, and only then a version bump here — a
//!   cross-repo, cross-release chain that cannot land as a single PR against
//!   either repo.
//! - **#1510 reaffirmed.** Both blockers this doc comment previously
//!   flagged as open are now closed (file path: aptu-coder#1459; call-edge
//!   parity: verified above). No new information surfaced that warrants
//!   reopening #1510's "keep separate" decision — the ontology, precedent,
//!   and release-topology reasons above still hold. A future proposal to
//!   consolidate would need its own issue. See issue #1525.
//!
//! See issue #1510 for the full analysis.

pub mod builder;
pub mod cache;
pub mod query;

pub use builder::build_from_analysis;
pub use query::blast_radius;

use serde::{Deserialize, Serialize};

/// A node in the structural graph, representing a source-code symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Node {
    /// A source file.
    File {
        /// File name (basename).
        name: String,
        /// File path relative to the repository root.
        path: String,
    },
    /// A module (e.g., a Rust `mod` declaration).
    Module {
        /// Module name.
        name: String,
        /// File path containing the module.
        path: String,
    },
    /// A function or method definition.
    Function {
        /// Function name.
        name: String,
        /// File path containing the function.
        path: String,
        /// Visibility (e.g., `pub`, `pub(crate)`, private).
        visibility: String,
    },
}

impl Node {
    /// Returns the display name of the node, regardless of variant.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Node::File { name, .. } | Node::Module { name, .. } | Node::Function { name, .. } => {
                name
            }
        }
    }

    /// Returns the file path associated with this node.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Node::File { path, .. } | Node::Module { path, .. } | Node::Function { path, .. } => {
                path
            }
        }
    }
}

/// A directed edge between two nodes in the structural graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edge {
    /// The source node contains the target node (e.g., file contains function).
    Contains,
    /// The source node calls the target node (function call).
    Calls,
    /// The source node imports the target node (module or item import).
    Imports,
}

/// The structural graph database: a directed graph of `Node`s connected by `Edge`s.
pub type GraphDb = petgraph::graph::DiGraph<Node, Edge>;
