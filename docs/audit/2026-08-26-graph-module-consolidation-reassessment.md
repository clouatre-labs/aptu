# Audit: Structural Graph Module Consolidation — Reassessment

Date: 2026-08-26
Commits: aptu `df25d1a` (origin/main), aptu-coder `f2ce662` (origin/main)
Toolchain: Rust 1.98.0, petgraph 0.8.3

## Purpose

Re-examine the decision recorded in PR #1512 (closing issue #1510) to keep
`aptu-core`'s structural graph module (`crates/aptu-core/src/graph/`) separate
from `aptu_coder_core::graph::StructuralGraph`. #1512's rationale is verified
here against current code in both repos, not against its own prose.

## Scope

- aptu: `crates/aptu-core/src/graph/{mod,builder,query,cache}.rs`, decision
  doc at `graph/mod.rs:1-46`.
- aptu-coder: `crates/aptu-coder-core/src/graph/structural.rs`,
  `docs/audit/2026-08-24-knowledge-graph-implementation.md`.
- Method: direct code inspection (grep for construction sites of every
  `Node`/`Edge` variant in both repos), `git log --follow` for provenance,
  `Cargo.lock`/crates.io registry check for dependency topology.

---

## Findings

### F1: The ontology-divergence justification in #1512 is inaccurate (HIGH / DOC)

**Severity:** High
**Category:** Stale decision rationale
**Tracking:** issue #1520

`aptu-core/src/graph/mod.rs:26-31` states: "This module *does* emit and query
Rust-specific `Struct`/`Enum`/`Trait`/`Impl` nodes and
`Implements`/`HasMethod`/`Tests` edges below."

**Verified false.** `build_from_analysis` (`builder.rs:40-124`) — the only
function that constructs a graph in production — emits exactly
`Node::File`, `Node::Function`, `Node::Module` and `Edge::Contains`,
`Edge::Imports`, `Edge::Calls`. A grep across
`crates/aptu-core/src/graph/*.rs` for
`Node::Struct|Node::Enum|Node::Trait|Node::Impl|Edge::Implements|Edge::HasMethod|Edge::Tests`
returns zero construction sites — only match arms in `query.rs:61,164,180-183`
and field accessors in `mod.rs:127-144` that reference the variants without
ever receiving an instance.

**Provenance:** `git log --follow -- crates/aptu-core/src/graph/builder.rs`
shows PR #1420 shipped the original graph feature with a text-format-parsing
builder, and PR #1445 ("refactor(graph): build `GraphDb` directly from
aptu-coder-core structs; delete text-format parsers") replaced it with the
current typed-struct builder. The richer node/edge kinds were emitted by the
deleted text-parser; #1445 dropped that emission when switching to typed
input but did not trim the `Node`/`Edge` enum to match. #1512 was written
after #1445 and describes the pre-#1445 implementation, not the code it was
merged against. **PR #1445 is entirely within the aptu repository — this is
not an aptu-coder defect.**

**Impact:** the entire ontology-divergence argument against consolidation —
that `aptu-coder-core`'s deliberately-narrowed `StructuralGraph` enum is
insufficient for aptu's needs — rests on a false premise. aptu's actual
(non-dead) ontology already equals `StructuralGraph`'s ontology exactly: File
/ Function-as-Symbol / Module nodes; Contains / Calls / Imports edges.

### F2: aptu's graph module carries the same dead-ontology defect aptu-coder-core already fixed as its own F3 (MEDIUM / DESIGN)

**Severity:** Medium
**Category:** Dead code
**Tracking:** issue #1521

Same defect class, same root cause, independently present in both repos at
different times. aptu-coder-core's own audit
(`docs/audit/2026-08-24-knowledge-graph-implementation.md`, F3) found
`StructuralGraph`'s `Edge` enum declared 6 variants but emitted only 3, and
fixed it in PR #1438 by removing the dead variants and bumping
`FORMAT_VERSION`. aptu's `graph::Node`/`Edge` are in that same state today
and have not been audited:

- Dead `Node` variants: `Struct`, `Enum`, `Trait`, `Impl` (declared
  `mod.rs:82-113`, matched `query.rs:180-183`, never constructed).
- Dead `Edge` variants: `Implements`, `HasMethod`, `Tests` (declared, matched
  `query.rs:61,164`, never constructed).
- `Edge::Modifies` is a distinct case: constructed only inside
  `#[cfg(test)] mod tests` in `cache.rs` (a test asserts it is stripped
  before a graph is cached), but no production code path ever adds it —
  `find_modified_nodes` (`query.rs:21`) returns a plain `Vec<NodeIndex>`, not
  graph edges. This reads as infrastructure built ahead of a caller that was
  never wired up, not pure leftover — it warrants a deliberate keep-or-remove
  decision rather than blanket deletion alongside the other six.

**Impact:** 7 of 10 declared `Node`/`Edge` variants are dead code with zero
construction sites anywhere in the crate. They inflate `cache.rs`'s
`SCHEMA_STRING` (`cache.rs:31`) and the serialized enum without producing any
graph data.

### F3: The real remaining blockers to consolidation are input-shape gaps, not ontology or release topology (INFO)

**Severity:** Info
**Category:** Architecture

With F1 established, the release-topology argument in #1512 (`aptu-coder-core`
is a crates.io dependency, so extending its ontology needs a cross-repo
release before aptu could consume it) does not apply: no ontology extension
is needed, since aptu's real ontology is already a subset of
`StructuralGraph`'s. `Cargo.lock` already resolves `aptu-coder-core` to
`0.30.1`, the current published version, against the `"0.30.0"` pin in
`aptu-core/Cargo.toml:37`.

Two real, smaller gaps remain if consolidation is pursued:

- `StructuralGraph::build_from_analysis` derives the file path from
  `entry.formatted.lines().next()` (`aptu-coder-core/src/graph/structural.rs:77`)
  rather than a struct field on `FileAnalysisOutput` — an implicit text
  convention a consumer would need to replicate exactly.
- aptu currently sources call edges from
  `aptu_coder_core::graph::CallGraph.callers`, filtering `<reference>`
  pseudo-edges and `is_impl_trait` edges (`builder.rs:88-96`);
  `StructuralGraph::build_from_analysis` sources from `entry.semantic.calls`
  directly with no equivalent filtering. Behavioral parity between the two
  paths is unverified and would need parity tests before any swap, per
  #1510's own acceptance criteria (`blast_radius()` output must not change).

### F4: Graph feature is already compiled and runtime-gated correctly (INFO)

**Severity:** Info
**Category:** Positive

Unrelated to consolidation: the `graph` Cargo feature has been requested by
`aptu-cli` since PR #1511 (`aptu-cli/Cargo.toml:18`) and is compiled into the
default release build. It is disabled at runtime by default
(`GraphConfig::enabled` defaults to `false`, `config/graph.rs:36-38`) and
enabled via `[graph] enabled = true` in `config.toml`. Confirmed present in
the locally installed binary via `strings` inspection.

---

## Recommendations

### R1: Correct #1512's decision doc (fixes F1)

**Priority:** High

The doc comment in `aptu-core/src/graph/mod.rs:11-46` should be corrected to
match current code, and the keep-separate decision re-affirmed or reversed
for the reasons in F3 — not the false ontology claim currently on record.

### R2: Remove the 7 unemitted Node/Edge variants; decide `Modifies` separately (fixes F2)

**Priority:** Medium

Mirrors aptu-coder-core PR #1438: remove `Struct`/`Enum`/`Trait`/`Impl`/
`Implements`/`HasMethod`/`Tests`, bump the schema hash via the existing
auto-invalidation mechanism (`cache.rs:35`, documented at
`docs/CONFIGURATION.md:309`), update `SCHEMA_STRING`. Decide separately
whether `Modifies` is forward-looking design (keep, document its intended
caller) or vestigial (remove with the rest).

### R3: If re-evaluating consolidation, scope against F3's real gaps

**Priority:** Info

Not prescribed here. If pursued: bridge the file-path convention and verify
call-edge parity between `CallGraph`-sourced and `SemanticAnalysis.calls`-
sourced edges with tests, before removing aptu's own builder/query code.

---

## Summary

*Table 1: Findings.*

| ID | Severity | Category | Finding | Status |
|---|---|---|---|---|
| F1 | High | DOC | #1512's ontology-divergence justification is false against current code (verified: `build_from_analysis` never emits the claimed variants) | OPEN (#1520) |
| F2 | Medium | DESIGN | 7 of 10 `Node`/`Edge` variants are dead code, same class as aptu-coder-core's already-fixed F3 | OPEN (#1521) |
| F3 | Info | ARCHITECTURE | Real blockers to consolidation are input-shape gaps (file-path convention, call-edge filtering parity), not ontology or release topology | INFO |
| F4 | Info | POSITIVE | `graph` feature already compiled and correctly runtime-gated since #1511 | INFO |

*Table 2: Recommendations.*

| ID | Priority | Fixes | Recommendation |
|---|---|---|---|
| R1 | High | F1 | Correct #1512's decision doc to match current code |
| R2 | Medium | F2 | Remove unemitted `Node`/`Edge` variants; decide `Modifies` separately |
| R3 | Info | — | If re-evaluating consolidation, scope against F3, not the retired ontology argument |

---

## aptu-coder: no fix needed for F1/F2

PR #1445 and the dead-ontology defect it introduced are entirely within the
aptu repository (`clouatre-labs/aptu/pull/1445`); there is nothing to change
in aptu-coder for F1 or F2.

One related weakness surfaced during this audit, relevant only if aptu (or
another consumer) later depends on `StructuralGraph` directly: the
file-path-from-`formatted`-text convention at `structural.rs:77` (F3) is
fragile and undocumented as a public contract. Suggested, not filed: add an
explicit `path: String` field to `FileAnalysisOutput`, or an alternate
`build_from_analysis` entry point taking paths explicitly, so callers don't
need to reverse-engineer the first line of `formatted`.
