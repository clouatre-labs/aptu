// SPDX-License-Identifier: Apache-2.0

//! Structural call graph for PR review context.
//!
//! Builds a petgraph-backed directed graph of source-code symbols (functions,
//! structs, enums, traits, impls) by parsing the rendered `<ast_context>` and
//! `<call_graph>` strings already produced by `crate::ast_context`, caches the
//! result on disk keyed by repository and commit SHA, and computes a bounded
//! blast-radius subgraph around modified symbols for prompt injection.

pub mod builder;
pub mod cache;
pub mod query;

pub use builder::{parse_ast_context_string, parse_call_graph_string};
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
    /// A struct definition.
    Struct {
        /// Struct name.
        name: String,
        /// File path containing the struct.
        path: String,
        /// Visibility (e.g., `pub`, `pub(crate)`, private).
        visibility: String,
    },
    /// An enum definition.
    Enum {
        /// Enum name.
        name: String,
        /// File path containing the enum.
        path: String,
        /// Visibility (e.g., `pub`, `pub(crate)`, private).
        visibility: String,
    },
    /// A trait definition.
    Trait {
        /// Trait name.
        name: String,
        /// File path containing the trait.
        path: String,
        /// Visibility (e.g., `pub`, `pub(crate)`, private).
        visibility: String,
    },
    /// An `impl` block.
    Impl {
        /// Impl target name (type or trait-for-type description).
        name: String,
        /// File path containing the impl block.
        path: String,
    },
}

impl Node {
    /// Returns the display name of the node, regardless of variant.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Node::File { name, .. }
            | Node::Module { name, .. }
            | Node::Function { name, .. }
            | Node::Struct { name, .. }
            | Node::Enum { name, .. }
            | Node::Trait { name, .. }
            | Node::Impl { name, .. } => name,
        }
    }

    /// Returns the file path associated with this node.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Node::File { path, .. }
            | Node::Module { path, .. }
            | Node::Function { path, .. }
            | Node::Struct { path, .. }
            | Node::Enum { path, .. }
            | Node::Trait { path, .. }
            | Node::Impl { path, .. } => path,
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
    /// The source node (impl) implements the target node (trait or type).
    Implements,
    /// The source node (impl) has the target node as a method.
    HasMethod,
    /// The source node modifies the target node (ephemeral; never cached).
    Modifies,
    /// The source node (test function) tests the target node.
    Tests,
}

/// The structural graph database: a directed graph of `Node`s connected by `Edge`s.
pub type GraphDb = petgraph::graph::DiGraph<Node, Edge>;
