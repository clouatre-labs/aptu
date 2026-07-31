---
title: "Add petgraph-based in-process structural graph context to pr review"
intent: "aptu pr review injects a bounded, cached call/dependency subgraph for modified symbols into the AI prompt, improving structural grounding without breaking the WASM build."
acceptance_criteria:
  - "cargo build succeeds with default features and with --features graph"
  - "cargo check -p aptu-core --target wasm32-unknown-unknown --no-default-features succeeds (graph module fully cfg-gated out)"
  - "aptu-core::graph module exists with builder.rs, cache.rs, query.rs, mod.rs implementing the node/edge schema defined below"
  - "Graph cache file is written to ~/.local/share/aptu/graph/<repo>/<sha>.bin using bincode, keyed by commit SHA"
  - "provider/review.rs injects the blast-radius subgraph into ReviewContext via the existing ast_context/call_graph string fields, subject to ReviewConfig::max_prompt_chars"
  - "[graph] config section is parsed from ~/.config/aptu/config.toml with enabled, cache_ttl_hours, max_nodes fields and documented in docs/CONFIGURATION.md"
  - "Unit tests cover: graph builder round-trip on a fixture Rust file, blast-radius BFS on a synthetic 3-node graph, cache hit and cache miss paths"
  - "Integration test: a fixture PR touching one function with two known callers produces a subgraph containing exactly those two callers"
  - "cargo clippy -- -D warnings and cargo fmt --check pass on all new and modified files"
  - "No panics on malformed or macro-heavy Rust input; unparseable nodes are skipped with a tracing::warn"
scope_boundary:
  files_owned:
    - "crates/aptu-core/src/graph/mod.rs"
    - "crates/aptu-core/src/graph/builder.rs"
    - "crates/aptu-core/src/graph/cache.rs"
    - "crates/aptu-core/src/graph/query.rs"
    - "crates/aptu-core/src/ai/provider/review.rs"
    - "crates/aptu-core/src/ai/review_context.rs"
    - "crates/aptu-core/src/config/review.rs"
    - "crates/aptu-core/src/config/mod.rs"
    - "crates/aptu-core/src/config/loader.rs"
    - "crates/aptu-core/Cargo.toml"
    - "docs/CONFIGURATION.md"
  files_readonly:
    - "crates/aptu-core/src/ai/prompts/mod.rs"
    - "crates/aptu-core/src/ai/prompts/pr_review_guidelines.md"
    - "crates/aptu-cli/"
complexity_signal: sonnet
line_budget: 500
dependencies: []
checkpoints:
  - after: 3
    reviewers: [coder-review, coder-qa]
  - after: 6
    reviewers: [coder-review, coder-qa]
---

## Purpose

`aptu pr review` currently injects AST context and an optional call graph into the
review prompt as flat strings (`ReviewContext::ast_context`, `ReviewContext::call_graph`,
built in `crates/aptu-core/src/ai/review_context.rs` and consumed by
`build_pr_review_user_prompt` in `crates/aptu-core/src/ai/prompts/mod.rs`). That context
is produced by a linear pass over changed files with no structural relationship between
symbols; the model receives text snippets, not a graph.

This spec adds an in-process, petgraph-backed structural graph of the repository (files,
modules, functions, structs, enums, traits, impls, and the edges between them) built at
review time via tree-sitter parsing of PR-relevant files. The graph is used to compute
the blast radius of each modified symbol (BFS over `Calls`, `Implements`, and `Tests`
edges), and the resulting bounded subgraph is serialized to structured text and injected
into the existing `ast_context` / `call_graph` prompt fields, unchanged in shape from the
model's perspective but with materially better signal on what else is affected by a change.

The graph is cached on disk per commit SHA so repeated reviews of the same commit (retry,
`--dry-run` iteration, CI re-run) do not re-parse the repository.

## Evidence

- RepoGraph (ICLR 2025, arXiv:2410.14684) reports a 32.8% relative improvement in resolve
  rate on SWE-bench Lite when agents are given structural repository graph context versus
  flat file/snippet context.
- KGCompass (arXiv:2603.27277) reports 10x fewer tokens and 2.1x fewer tool calls at a
  58.3% resolve rate on SWE-bench Lite when grounding agent context in a knowledge graph
  rather than unstructured retrieval.
- Code-review-graph (deepwiki.com/tirth8205/code-review-graph) documents a median 82x
  per-question token reduction when review context is filtered through a dependency graph
  instead of passed as raw diffs plus full files.
- Blitzy's autonomous-coder methodology (spec section 11) treats a persistent knowledge
  graph as long-lived grounding for agent tasks, distinct from per-task prompt context;
  this spec scopes the graph to the lifetime of a single review (cached by SHA) rather
  than building a persistent whole-repository knowledge graph, since `aptu` is a
  single-shot CLI, not a long-running agent process.

These sources motivate structural graph context as a token-efficient, higher-signal
alternative to string-only AST/call-graph injection; they do not claim applicability to
aptu's specific prompt budgets or model set, which this spec's acceptance criteria and
tests must validate independently.

## Graph Schema

Implemented with `petgraph::graph::DiGraph<Node, Edge>`. Node and edge payload enums live
in `crates/aptu-core/src/graph/mod.rs`.

### Nodes

- `File { path: String, crate_name: String, module_path: String }`
- `Module { name: String, visibility: Visibility }`
- `Function { qualified_name: String, signature: String, visibility: Visibility, is_async: bool, is_unsafe: bool, byte_range: (usize, usize) }`
- `Struct { qualified_name: String, fields_count: usize, visibility: Visibility, derives: Vec<String> }`
- `Enum { qualified_name: String, variants_count: usize, visibility: Visibility }`
- `Trait { qualified_name: String, method_count: usize, visibility: Visibility }`
- `Impl { target_type: String, trait_ref: Option<String>, byte_range: (usize, usize) }`

`Visibility` is a small enum (`Public`, `Crate`, `Private`) shared across node variants.

### Edges

- `Contains`: `File -> Module`, `Module -> Function|Struct|Enum|Trait|Impl`
- `Calls`: `Function -> Function`
- `Imports`: `File -> File`, derived from `use` paths resolved to a file in the changed-file set or its direct dependencies
- `Implements`: `Impl -> Trait`, `Impl -> Struct|Enum` (target type)
- `HasMethod`: `Trait|Impl -> Function`
- `Modifies`: ephemeral, added only at review time; `PR diff hunk -> Function|Struct|Impl` for nodes overlapping a changed line range; never persisted to the on-disk cache
- `Tests`: `Function (test) -> Function`, inferred by same-module co-location plus a `Calls` edge to the target, or by direct call when no co-location heuristic applies

`Modifies` edges are computed fresh on every review from the PR's diff hunks and the
cached base graph; they are not part of the serialized cache payload, since they depend
on the diff, not the commit content alone.

## Implementation Steps

1. **Dependencies.** Add `petgraph`, `tree-sitter`, `tree-sitter-rust`, and `bincode` to
   `crates/aptu-core/Cargo.toml` under a new `graph` feature (`graph = ["dep:petgraph",
   "dep:tree-sitter", "dep:tree-sitter-rust", "dep:bincode"]`), listed only under
   `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` alongside `tokio`,
   `backon`, `octocrab`. The `graph` feature must not appear in `[features] default = [...]`;
   it is opt-in only. This ensures the feature is never activated in WASM builds that
   rely on `--no-default-features`. Follow the existing `ast-context` feature as the
   template for how an optional, non-wasm capability is declared. Document the feature
   flag and its effect in `docs/CONFIGURATION.md` alongside the `[graph]` config section
   (see step 4), including how to enable it at build time (`--features graph`) and the
   fact that it is excluded from the default binary distribution.

2. **`aptu-core::graph` module.**
   - `mod.rs`: node/edge enums, `Visibility`, `pub type StructuralGraph = petgraph::graph::DiGraph<Node, Edge>`, re-exports.
   - `builder.rs`: `build_graph(files: &[PrFile]) -> StructuralGraph` — parses each file's
     content with `tree-sitter-rust`, walks the AST, and inserts nodes/edges per the
     schema above. Non-Rust files are skipped for node extraction (no graph coverage
     claimed) but still get a `File` node so `Imports` edges can resolve. Any tree-sitter
     parse error or node it cannot classify is skipped with `tracing::warn!`, never a
     panic or `Result::Err` that aborts the whole build.
   - `cache.rs`: `load_or_build(repo: &str, sha: &str, files: &[PrFile]) -> StructuralGraph`
     — checks `~/.local/share/aptu/graph/<repo>/<sha>.bin` (via `config::data_dir()` joined
     with `graph/<repo>/<sha>.bin`); on hit within `cache_ttl_hours`, deserializes with
     `bincode`; on miss, calls `builder::build_graph`, writes the result, and returns it.
     Cache path sanitizes `repo` (owner/name) to a filesystem-safe form consistent with
     existing cache key handling in `aptu-core::cache`. The serialized payload must begin
     with a `u32` format version field (current value: `1`); on deserialization, if the
     stored version does not match the current constant, the cache file is discarded and
     rebuilt. This allows future node/edge schema changes to invalidate stale cache files
     automatically without requiring a manual cache purge.
   - `query.rs`: `blast_radius(graph: &StructuralGraph, seeds: &[NodeIndex], max_nodes: usize) -> StructuralGraph`
     — bounded BFS over `Calls`, `Implements`, `HasMethod`, `Tests` edges (both directions
     for `Calls`, so callers and callees of a modified function are included) starting
     from nodes touched by `Modifies` edges, capped at `max_nodes`; also
     `render_subgraph_text(sub: &StructuralGraph) -> String` producing the structured text
     block injected into the prompt (one line per node with its edges, grouped by file).

3. **Wire into `provider/review.rs`.** After PR file contents are fetched (existing flow
   in `review_pr` / `build_review_context`), if `GraphConfig::enabled` and a repo path and
   commit SHA are resolvable: load or build the graph via `cache::load_or_build`, add
   `Modifies` edges from the PR's diff hunks, run `query::blast_radius` on the touched
   nodes, render the subgraph, and inject the resulting text into `ReviewContext` using
   the existing `ast_context` and `call_graph` fields (append rather than replace, so
   existing string-based context and this addition coexist under one budget). This
   reuses the existing budget-drop pipeline in `review_context.rs`
   (`apply_budget_drops`) without adding a new drop category; if that proves
   insufficient during implementation, a `graph_context` field and matching drop entry
   may be added, but the injection point itself is `ast_context`/`call_graph` per the
   parent issue's framing.

4. **Config.** Add `GraphConfig` to `crates/aptu-core/src/config/review.rs` (or a
   sibling `crates/aptu-core/src/config/graph.rs`, at implementer's discretion, wired
   into `AppConfig` in `config/mod.rs` the same way `ReviewConfig` is): `enabled: bool`
   (default `true`), `cache_ttl_hours: u64` (default `24`), `max_nodes: usize` (default
   `50_000`). Document the new `[graph]` section in `docs/CONFIGURATION.md` alongside the
   existing `[review]` and `[cache]` sections, including defaults and the effect of each
   field.

5. **Unit tests.**
   - Graph builder round-trip: parse a small fixture Rust source string with two
     functions where one calls the other; assert the resulting graph has the expected
     `Function` nodes and a `Calls` edge between them.
   - Blast-radius query: construct a synthetic 3-node graph by hand (`A -> B -> C` via
     `Calls`) and assert `blast_radius` from `A` returns `{A, B, C}` within
     `max_nodes = 10`, and returns only `{A, B}` when `max_nodes = 2`.
   - Cache hit/miss: write a graph via `cache::load_or_build` to a temp `data_dir`
     (override via existing test patterns for XDG paths in the codebase), assert the
     second call for the same `(repo, sha)` does not re-parse (e.g., by asserting file
     mtime is unchanged or by counting builder invocations through a test-only hook).

6. **Integration test.** Add a fixture under a test-data directory with one function
   that has two known callers in the same crate; drive `provider::review` (or the
   narrowest function that wires graph injection) with a synthetic `PrDetails`/`PrFile`
   set touching only that function, and assert the rendered subgraph text in the
   resulting `ReviewContext` contains both caller qualified names and does not contain
   unrelated functions from the fixture.

7. **WASM gate.** Wrap the entire `graph` module declaration in
   `crates/aptu-core/src/lib.rs` (or wherever modules are declared) with
   `#[cfg(not(target_arch = "wasm32"))]`, matching the pattern already used for
   `keyring`, `process::Command`, `tokio`, and `backon` per the WASM Portability
   conventions for this repository. Verify with
   `cargo check -p aptu-core --target wasm32-unknown-unknown --no-default-features`.

## Risks

- **tree-sitter grammar completeness.** Macro-heavy Rust (derive macros, proc macros,
  `macro_rules!` bodies) may not decompose into the expected node shapes.
  Mitigation: the builder never panics on an unrecognized or malformed node; it skips the
  node and emits `tracing::warn!`, so coverage degrades gracefully rather than failing
  the review.
- **Cache invalidation on force-push.** A cache key of `<repo>/<sha>.bin` is stable for
  an immutable commit, but a force-pushed branch can reuse a SHA-adjacent state
  incorrectly only if the SHA itself is reused, which does not happen for distinct
  commits. Mitigation: `cache_ttl_hours` bounds staleness exposure, and the cache value
  additionally stores a hash of the input file list so a `(repo, sha)` collision against
  a differently-scoped file set is detected and forces a rebuild.
- **WASM build regressions.** `petgraph` and `tree-sitter` both carry assumptions that
  may not hold under `wasm32-unknown-unknown` with `--no-default-features`.
  Mitigation: the `graph` feature is not part of `default`, the module is fully
  `cfg`-gated out on `wasm32`, and the existing `wasm-check` CI job
  (`cargo check -p aptu-core --target wasm32-unknown-unknown --no-default-features`)
  is the acceptance gate for this risk.
- **Prompt budget overrun.** Subgraph text must not silently blow past
  `ReviewConfig::max_prompt_chars`. Mitigation: `query::blast_radius` takes a hard
  `max_nodes` cap from `GraphConfig::max_nodes`, and injected text still flows through
  the existing `apply_budget_drops` pipeline in `review_context.rs`, so oversized output
  is dropped under the same rules as today's `ast_context`/`call_graph` content.

## Acceptance Criteria

See YAML front matter for the authoritative, verifiable list. Summary:

- Builds cleanly with default features and with `graph` enabled; WASM check passes with
  the graph module excluded.
- `aptu-core::graph` implements the schema above with builder, cache, and query
  submodules.
- Cache files land at `~/.local/share/aptu/graph/<repo>/<sha>.bin`, `bincode`-encoded.
- `provider/review.rs` injects a bounded blast-radius subgraph into the existing
  `ast_context`/`call_graph` prompt fields, respecting `ReviewConfig::max_prompt_chars`.
- `[graph]` config section is parsed and documented with its three fields and defaults.
- Unit and integration tests per Implementation Steps 5 and 6 pass under `cargo test`.
- `cargo clippy -- -D warnings` and `cargo fmt --check` pass.
- No panics on malformed input; parse failures degrade to skipped nodes with a warning.

## Dependencies

None. This feature can be implemented independently of model routing work and prompt
schema minification efforts; it only touches the review-context assembly path and adds
a new, feature-gated module.
