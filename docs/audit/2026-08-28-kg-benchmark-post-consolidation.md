# Audit: KG Benchmark — Post-Consolidation (StructuralGraph) — August 2026

> Companion to `2026-08-27-kg-benchmark-baseline.md` (pre-consolidation baseline, Tables 1-13).
> This is the first benchmark of the consolidated graph implementation (PR #1544: local
> `graph::builder` + `graph::query` retired in favor of `aptu-coder-core::graph::StructuralGraph`).

Date: 2026-08-28
Toolchain: aptu 0.10.16 release build; main @ 0ec329b; aptu-coder-core 0.32.3 (Cargo.lock)
Data: 4 PRs x 2 configs x 3 runs = 24 total runs, OpenRouter `mistralai/mistral-small-2603` on all runs.
Method: `aptu pr review <PR> --repo clouatre-labs/aptu -o json` — real AI calls, nothing posted.
Scope: `~/.config/aptu/config.toml` (`[graph]` section), `~/.local/share/aptu/graph/` (disk cache),
`.ai_stats` JSON fields (`prompt_chars`, `input_tokens`, `cost_usd`, `duration_ms`, `model`),
`.review` JSON fields (`verdict`, `comments`, `concerns`, `strengths`).

---

## Purpose

The baseline document measured KG cost on the pre-consolidation implementation (builds at 88c20e4
and 32f546d, both before PR #1544). No benchmark data existed for the consolidated
`StructuralGraph` implementation. This audit re-runs the identical method against current main to
verify three invariants before release:

1. F6 invariant (baseline): KG injects non-empty context on all 4 PRs.
2. F4 invariant (baseline): graph cache produces identical prompt chars across cold/warm runs.
3. Token overhead stays proportional to graph-indexed symbol density in modified files.

Post-consolidation PRs #1552, #1553, and #1554 also changed review-context composition, so the
No-KG baseline itself is re-measured in the same build. Within-build No KG vs KG is the primary
comparison axis (per R4 of the baseline: `input_tokens` is the primary metric).

## Method

Identical to the baseline document: 4 merged PRs (#1529, #1531, #1532, #1519), 2 configs
(No KG: no config file; KG: `[graph]` config below), 3 runs per PR per config. Graph cache cleared
between configs only, never between runs of the same config. Run 1 of each config per PR is cold
(graph cache build); runs 2-3 are warm (cache hit).

```toml
[graph]
enabled = true
max_depth = 4
max_nodes = 50000
cache_ttl_hours = 24
```

All runs invoked the release binary built from main @ 0ec329b
(`cargo install --path crates/aptu-cli --profile release`).

## Results

### Table 1: All 24 Runs

| PR | Config | Run | Prompt Chars | Input Tokens | Cost (USD) | Duration (ms) | Verdict | Comments | Concerns | Strengths |
|------|--------|-----|-------------|-------------|------------|---------------|---------|----------|----------|-----------|
| 1529 | No KG | 1 | 16,661 | 5,454 | $0.000890 | 1,602 | approve | 0 | 0 | 4 |
| 1529 | No KG | 2 | 16,661 | 5,466 | $0.001140 | 1,557 | approve | 0 | 0 | 4 |
| 1529 | No KG | 3 | 16,661 | 5,466 | $0.001188 | 2,504 | approve | 0 | 0 | 3 |
| 1529 | KG | 1 | 16,661 | 5,466 | $0.001143 | 1,830 | approve | 0 | 0 | 4 |
| 1529 | KG | 2 | 16,661 | 5,454 | $0.000776 | 2,038 | approve | 0 | 0 | 4 |
| 1529 | KG | 3 | 16,661 | 5,454 | $0.000171 | 1,519 | approve | 0 | 0 | 4 |
| 1531 | No KG | 1 | 72,442 | 20,605 | $0.003289 | 3,246 | approve | 0 | 0 | 5 |
| 1531 | No KG | 2 | 72,442 | 20,617 | $0.004071 | 3,172 | approve | 0 | 0 | 5 |
| 1531 | No KG | 3 | 72,442 | 20,605 | $0.000568 | 3,982 | approve | 2 | 0 | 5 |
| 1531 | KG | 1 | 75,134 | 21,369 | $0.003326 | 4,083 | approve | 2 | 0 | 5 |
| 1531 | KG | 2 | 72,442 | 20,605 | $0.000636 | 3,780 | approve | 1 | 0 | 7 |
| 1531 | KG | 3 | 72,442 | 20,605 | $0.000500 | 2,500 | approve | 0 | 0 | 6 |
| 1532 | No KG | 1 | 81,488 | 28,527 | $0.005796 | 5,841 | approve | 4 | 0 | 6 |
| 1532 | No KG | 2 | 81,488 | 28,527 | $0.005776 | 4,646 | approve | 2 | 0 | 5 |
| 1532 | No KG | 3 | 81,488 | 28,515 | $0.004453 | 5,472 | approve | 2 | 0 | 5 |
| 1532 | KG | 1 | 81,865 | 28,614 | $0.004467 | 4,945 | approve | 2 | 0 | 6 |
| 1532 | KG | 2 | 81,488 | 28,515 | $0.000726 | 3,474 | approve | 2 | 0 | 5 |
| 1532 | KG | 3 | 81,488 | 28,515 | $0.004505 | 4,277 | approve | 1 | 0 | 5 |
| 1519 | No KG | 1 | 72,972 | 19,243 | $0.002990 | 4,393 | approve | 2 | 0 | 5 |
| 1519 | No KG | 2 | 72,972 | 19,243 | $0.000675 | 5,551 | approve | 3 | 0 | 6 |
| 1519 | No KG | 3 | 72,972 | 19,243 | $0.000558 | 3,674 | approve | 2 | 0 | 6 |
| 1519 | KG | 1 | 81,913 | 21,642 | $0.003494 | 6,150 | approve | 4 | 0 | 6 |
| 1519 | KG | 2 | 72,972 | 19,243 | $0.000675 | 4,893 | approve | 2 | 0 | 6 |
| 1519 | KG | 3 | 72,972 | 19,243 | $0.000619 | 4,587 | approve | 3 | 0 | 6 |

Note: ±12 input-token jitter appears on identical prompt chars (e.g., No KG #1529: 5,454 vs
5,466), consistent with tokenizer variance observed in the baseline.

### Table 2: Per-PR Cold/Warm Summary (No KG reference = warm average, runs 2+3)

| PR | No KG chars | No KG tokens | No KG dur (ms) | KG cold chars | KG cold tokens | KG cold dur (ms) | KG warm chars | KG warm tokens | KG warm dur (ms) | KG cold token delta |
|------|------------|-------------|----------------|---------------|----------------|------------------|---------------|----------------|------------------|---------------------|
| 1519 | 72,972 | 19,243 | 4,613 | 81,913 | 21,642 | 6,150 | 72,972 | 19,243 | 4,740 | +12.47% |
| 1529 | 16,661 | 5,466 | 2,031 | 16,661 | 5,466 | 1,830 | 16,661 | 5,454 | 1,779 | 0% |
| 1531 | 72,442 | 20,611 | 3,577 | 75,134 | 21,369 | 4,083 | 72,442 | 20,605 | 3,140 | +3.68% |
| 1532 | 81,488 | 28,521 | 5,059 | 81,865 | 28,614 | 4,945 | 81,488 | 28,515 | 3,876 | +0.33% |

The dominant pattern: KG cold run (run 1) injects graph context; KG warm runs (runs 2-3) inject
exactly zero. KG warm prompt chars equal the No KG prompt chars to the character on all 4 PRs.

### Table 3: Graph Cache Files (keyed by PR head SHA)

| PR | Head SHA (prefix) | Post-Consolidation Cache | Pre-Consolidation Post-Fix Cache (baseline Table 8) |
|------|-------------------|--------------------------|------------------------------------------------------|
| 1529 | `823128f3` | 12 bytes (empty) | 2,590 bytes |
| 1531 | `ee7efafe` | 6,585 bytes | 13,896 bytes |
| 1532 | `af4e9ce4` | 480 bytes | 6,322 bytes |
| 1519 | `3c2b019a` | 10,460 bytes | 21,933 bytes |

### Table 4: Cold-Run KG Injection vs Pre-Consolidation Post-Fix Baseline

| PR | Post-Consolidation chars / tokens | Post-Consolidation token delta | Baseline chars / tokens (Table 8) | Baseline token delta |
|------|-----------------------------------|-------------------------------|-----------------------------------|----------------------|
| 1519 | +8,941 / +2,399 | +12.47% | +37,958 / +10,285 | +53.6% |
| 1531 | +2,692 / +758 | +3.68% | +17,187 / +4,984 | +24.2% |
| 1532 | +377 / +93 | +0.33% | +14,182 / +3,611 | +12.5% |
| 1529 | 0 / 0 | 0% | +5,317 / +1,374 | +23.5% |

### Quality Indicators

All 24 runs returned `approve` with 0 concerns. Comment counts (0-4) and strengths (3-7) vary
with no directional KG effect, consistent with the baseline's noise finding (Q3) and its
Limitations section: this benchmark measures cost, not value.

## Findings

### F9: Warm graph-cache hits inject zero context — decoded StructuralGraph loses its symbol index (BUG)

**Severity:** High
**Category:** BUG

On every PR, KG runs 2-3 (warm cache) produced prompt content identical to the No KG config,
while KG run 1 (cold cache) injected graph context (Table 2). The mechanism, verified in source:

- `aptu-coder-core` 0.32.3 `src/graph/structural.rs:42-47`: `StructuralGraph` has two fields;
  `symbol_index: HashMap<String, Vec<NodeIndex>>` carries `#[serde(skip)]`, so postcard omits it
  on encode and decodes it to an empty map.
- `structural.rs:508-517`: `find_symbols_all` looks up seeds exclusively in `symbol_index`; on a
  decoded graph it returns no seeds.
- `structural.rs:531-533`: `blast_radius_bidirectional` with empty seeds returns empty nodes;
  `render_subgraph_text` then renders an empty string.
- `crates/aptu-core/src/graph/cache.rs:56-74` encodes/decodes with raw postcard and no
  post-decode rebuild. Upstream `rebuild_symbol_index()` (`structural.rs:75-77`) exists but is
  `pub(crate)`; the roundtrip test in `cache.rs:182-187` asserts only node and edge counts,
  which survive because the petgraph field is serialized.

**Impact:** The F4 invariant (baseline) is broken: cold and warm runs no longer produce identical
context. KG value is limited to the first review of each PR head SHA (within 24h TTL); every
cache-hit review silently receives zero graph context. This is a regression against the
pre-consolidation implementation, where all 3 runs per PR per config had identical prompt chars.

### F10: KG is a steady-state no-op under the cache-hit path (BUG)

**Severity:** High
**Category:** BUG

A direct consequence of F9: for any PR reviewed more than once, or reviewed after its first
review populated the cache, KG contributes nothing while the graph build cost was already paid.
In this benchmark, 8 of 12 KG runs (runs 2-3 across 4 PRs) injected zero context.

**Impact:** KG cost/value measurement on cache-hit workloads is meaningless until F9 is fixed.

### F11: Cold-run injection volume is far below the pre-consolidation baseline (MEASUREMENT)

**Severity:** Info
**Category:** MEASUREMENT

Where injection occurs (cold runs only), volume is 21-95% lower than the baseline's post-fix
figures for the same PRs (Table 4): #1519 +8,941 chars vs +37,958; #1531 +2,692 vs +17,187;
#1532 +377 vs +14,182. PR #1529 built an empty graph (12-byte cache, zero injection even cold)
where the pre-consolidation builder produced a 2,590-byte graph for the same head SHA. The
No-KG prompt baseline itself also shifted (e.g., #1529: 16,661 vs 17,826 chars), consistent
with post-consolidation context changes in #1552/#1553/#1554.

**Impact:** The consolidated implementation renders materially different graph context than the
retired local implementation. Whether the reduced volume is a deliberate rendering change
(compact serialization) or a loss (fewer nodes/edges reached) was not determined in this audit.
The F5-era expectation that injection correlates with symbol density in modified files still
holds directionally on cold runs.

### F12: No quality signal; cost metrics confirm R4 (INFO)

**Severity:** Info
**Category:** MEASUREMENT

All 24 runs returned `approve` with 0 concerns; comment and strength counts fluctuate without a
directional KG effect, matching the baseline's Limitations analysis. Duration differences between
KG warm and No KG are within run-to-run noise. `cost_usd` again shows cold-run variance
unrelated to token counts, reconfirming R4 (use `input_tokens`).

## Recommendations

### R8: Fix symbol-index deserialization upstream before relying on the graph cache

**Priority:** High
**Fixes:** F9, F10

`StructuralGraph` in `aptu-coder-core` must rebuild `symbol_index` after deserialization
(serde post-deserialization hook or a public rebuild API; `rebuild_symbol_index()` already
exists as `pub(crate)`). In aptu, extend the roundtrip test in `crates/aptu-core/src/graph/cache.rs`
to assert `find_symbols_all` returns non-empty seeds after decode, not only node/edge counts.

### R9: Treat the graph feature as not release-verified for cache-hit usage

**Priority:** High
**Fixes:** F9, F10, F11

The graph feature is opt-in (`[graph] enabled = true`; default off), so default-configuration
behavior is unaffected. However, the consolidated implementation regressed both documented
invariants (F4: cache determinism; F6-equivalent: consistent injection), and PR #1529 now builds
an empty graph where the retired builder did not. KG default-enablement remains blocked per
baseline R6, and the consolidation's "preserving existing behavior" claim does not hold for the
cache-hit path. Re-run this benchmark after R8 to verify the F4 invariant before any release that
advertises the graph feature.

### R10: Re-run this benchmark after the upstream fix

**Priority:** Info
**Fixes:** F9

Same 4 PRs, same method. Acceptance: KG prompt chars identical across cold and warm runs per PR,
and warm-run injection equal to cold-run injection.

## Summary

*Table 5: Findings.*

| ID | Severity | Category | Finding |
|----|----------|----------|---------|
| F9 | High | BUG | Decoded StructuralGraph loses `symbol_index` (`#[serde(skip)]`); warm cache hits inject zero context |
| F10 | High | BUG | KG is a steady-state no-op on cache-hit reviews (8 of 12 KG runs) |
| F11 | Info | MEASUREMENT | Cold-run injection 21-95% below pre-consolidation baseline; #1529 builds an empty graph |
| F12 | Info | MEASUREMENT | No quality signal; input_tokens confirmed as primary metric |

*Table 6: Recommendations.*

| ID | Priority | Fixes | Recommendation |
|----|----------|-------|----------------|
| R8 | High | F9, F10 | Rebuild symbol index post-deserialize upstream; extend aptu roundtrip test |
| R9 | High | F9-F11 | Graph feature not release-verified for cache-hit usage; re-verify before release |
| R10 | Info | F9 | Re-run benchmark post-fix; acceptance: cold/warm injection identical |

## Reproduction

```bash
# Build from main
cargo install --path crates/aptu-cli --profile release

# No KG config: remove ~/.config/aptu/config.toml. KG config: the [graph] block above.
# Clear graph cache between configs only:
rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/

for pr in 1529 1531 1532 1519; do
  for run in 1 2 3; do
    aptu pr review $pr --repo clouatre-labs/aptu -o json 2>/dev/null \
      | jq '{prompt_chars: .ai_stats.prompt_chars, input_tokens: .ai_stats.input_tokens,
             cost_usd: .ai_stats.cost_usd, duration_ms: .ai_stats.duration_ms,
             model: .ai_stats.model, verdict: .review.verdict,
             comments: (.review.comments | length), concerns: (.review.concerns | length),
             strengths: (.review.strengths | length)}'
  done
done
```

Do NOT clear cache between runs. Cleanup: remove the config file and the graph cache directory.
