# Audit: KG Benchmark — Post-Consolidation (StructuralGraph) — August 2026

> Companion to `2026-08-27-kg-benchmark-baseline.md` (pre-consolidation baseline, Tables 1-13). This is the first benchmark of the consolidated graph implementation (PR #1544: local `graph::builder` + `graph::query` retired in favor of `aptu-coder-core::graph::StructuralGraph`).

Date: 2026-08-28

Toolchain: aptu 0.10.16 release build; main @ 0ec329b; aptu-coder-core 0.32.3 (Cargo.lock)

Data: 4 PRs x 2 configs x 3 runs = 24 total runs, OpenRouter `mistralai/mistral-small-2603` on all runs.

Method: `aptu pr review <PR> --repo clouatre-labs/aptu -o json` — real AI calls, nothing posted.

Scope: `~/.config/aptu/config.toml` (`[graph]` section), `~/.local/share/aptu/graph/` (disk cache), `.ai_stats` JSON fields (`prompt_chars`, `input_tokens`, `cost_usd`, `duration_ms`, `model`), `.review` JSON fields (`verdict`, `comments`, `concerns`, `strengths`).

---

## Purpose

The baseline document measured KG cost on the pre-consolidation implementation (builds at 88c20e4 and 32f546d, both before PR #1544). No benchmark data existed for the consolidated `StructuralGraph` implementation. This audit re-runs the identical method against current main to verify three invariants before release:

1. F6 invariant (baseline): KG injects non-empty context on all 4 PRs.
2. F4 invariant (baseline): graph cache produces identical prompt chars across cold/warm runs.
3. Token overhead stays proportional to graph-indexed symbol density in modified files.

Post-consolidation PRs #1552, #1553, and #1554 also changed review-context composition, so the No-KG baseline itself is re-measured in the same build. Within-build No KG vs KG is the primary comparison axis (per R4 of the baseline: `input_tokens` is the primary metric).

## Method

Identical to the baseline document: 4 merged PRs (#1529, #1531, #1532, #1519), 2 configs (No KG: no config file; KG: `[graph]` config below), 3 runs per PR per config. Graph cache cleared between configs only, never between runs of the same config. Run 1 of each config per PR is cold (graph cache build); runs 2-3 are warm (cache hit).

```toml
[graph]
enabled = true
max_depth = 4
max_nodes = 50000
cache_ttl_hours = 24
```

All runs invoked the release binary built from main @ 0ec329b (`cargo install --path crates/aptu-cli --profile release`).

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

Note: ±12 input-token jitter appears on identical prompt chars (e.g., No KG #1529: 5,454 vs 5,466), consistent with tokenizer variance observed in the baseline.

### Table 2: Per-PR Cold/Warm Summary (No KG reference = warm average, runs 2+3)

| PR | No KG chars | No KG tokens | No KG dur (ms) | KG cold chars | KG cold tokens | KG cold dur (ms) | KG warm chars | KG warm tokens | KG warm dur (ms) | KG cold token delta |
|------|------------|-------------|----------------|---------------|----------------|------------------|---------------|----------------|------------------|---------------------|
| 1519 | 72,972 | 19,243 | 4,613 | 81,913 | 21,642 | 6,150 | 72,972 | 19,243 | 4,740 | +12.47% |
| 1529 | 16,661 | 5,466 | 2,031 | 16,661 | 5,466 | 1,830 | 16,661 | 5,454 | 1,779 | 0% |
| 1531 | 72,442 | 20,611 | 3,577 | 75,134 | 21,369 | 4,083 | 72,442 | 20,605 | 3,140 | +3.68% |
| 1532 | 81,488 | 28,521 | 5,059 | 81,865 | 28,614 | 4,945 | 81,488 | 28,515 | 3,876 | +0.33% |

The dominant pattern: KG cold run (run 1) injects graph context; KG warm runs (runs 2-3) inject exactly zero. KG warm prompt chars equal the No KG prompt chars to the character on all 4 PRs.

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

All 24 runs returned `approve` with 0 concerns. Comment counts (0-4) and strengths (3-7) vary with no directional KG effect, consistent with the baseline's noise finding (Q3) and its Limitations section: this benchmark measures cost, not value.

## Findings

### F9: Warm graph-cache hits inject zero context — decoded StructuralGraph loses its symbol index (BUG)

**Severity:** High

**Category:** BUG

On every PR, KG runs 2-3 (warm cache) produced prompt content identical to the No KG config, while KG run 1 (cold cache) injected graph context (Table 2). The mechanism, verified in source:

- `aptu-coder-core` 0.32.3 `src/graph/structural.rs:42-47`: `StructuralGraph` has two fields; `symbol_index: HashMap<String, Vec<NodeIndex>>` carries `#[serde(skip)]`, so postcard omits it on encode and decodes it to an empty map.
- `structural.rs:508-517`: `find_symbols_all` looks up seeds exclusively in `symbol_index`; on a decoded graph it returns no seeds.
- `structural.rs:531-533`: `blast_radius_bidirectional` with empty seeds returns empty nodes; `render_subgraph_text` then renders an empty string.
- `crates/aptu-core/src/graph/cache.rs:56-74` encodes/decodes with raw postcard and no post-decode rebuild. Upstream `rebuild_symbol_index()` (`structural.rs:75-77`) exists but is `pub(crate)`; the roundtrip test in `cache.rs:182-187` asserts only node and edge counts, which survive because the petgraph field is serialized.

**Impact:** The F4 invariant (baseline) is broken: cold and warm runs no longer produce identical context. KG value is limited to the first review of each PR head SHA (within 24h TTL); every cache-hit review silently receives zero graph context. This is a regression against the pre-consolidation implementation, where all 3 runs per PR per config had identical prompt chars.

### F10: KG is a steady-state no-op under the cache-hit path (BUG)

**Severity:** High

**Category:** BUG

A direct consequence of F9: for any PR reviewed more than once, or reviewed after its first review populated the cache, KG contributes nothing while the graph build cost was already paid. In this benchmark, 8 of 12 KG runs (runs 2-3 across 4 PRs) injected zero context.

**Impact:** KG cost/value measurement on cache-hit workloads is meaningless until F9 is fixed.

### F11: Cold-run injection volume is far below the pre-consolidation baseline (MEASUREMENT)

**Severity:** Info

**Category:** MEASUREMENT

Where injection occurs (cold runs only), volume is 21-95% lower than the baseline's post-fix figures for the same PRs (Table 4): #1519 +8,941 chars vs +37,958; #1531 +2,692 vs +17,187; #1532 +377 vs +14,182. PR #1529 built an empty graph (12-byte cache, zero injection even cold) where the pre-consolidation builder produced a 2,590-byte graph for the same head SHA. The No-KG prompt baseline itself also shifted (e.g., #1529: 16,661 vs 17,826 chars), consistent with post-consolidation context changes in #1552/#1553/#1554.

**Impact:** The consolidated implementation renders materially different graph context than the retired local implementation. Whether the reduced volume is a deliberate rendering change (compact serialization) or a loss (fewer nodes/edges reached) was not determined in this audit. The F5-era expectation that injection correlates with symbol density in modified files still holds directionally on cold runs.

### F12: No quality signal; cost metrics confirm R4 (INFO)

**Severity:** Info

**Category:** MEASUREMENT

All 24 runs returned `approve` with 0 concerns; comment and strength counts fluctuate without a directional KG effect, matching the baseline's Limitations analysis. Duration differences between KG warm and No KG are within run-to-run noise. `cost_usd` again shows cold-run variance unrelated to token counts, reconfirming R4 (use `input_tokens`).

### F13: Cold-run volume gap is a replay-methodology artifact, not a StructuralGraph regression (DETERMINATION)

**Severity:** Info

**Category:** MEASUREMENT

Follow-up to F11, closing out issue #1557. `ast_context.rs::build_ast_context_sync` reads every changed file from the local filesystem at `repo_path` via `analyze_file`, never from the PR's own historical commit through GitHub's Contents API. `derive_modified_symbols` resolves diff-hunk line numbers exactly as recorded in the PR's own historical patch (fetched from GitHub) against `symbol_ranges` built from whatever is *currently checked out* on disk. All 4 benchmarked PRs were merged well before either benchmark's reference commit, so replaying them measures the checked-out file's present-day content, not the file as it stood at the PR's own head SHA.

This fully explains PR #1529's empty (12-byte) cache: its only changed file, `crates/aptu-core/src/graph/query.rs`, was deleted from main by PR #1544 — the very consolidation this audit measures. Post-consolidation, `analyze_file` cannot find the file at all; the error is silently swallowed (`ast_context.rs`: `Err(e) => debug!("ast_context: skipping {}: {}", file.filename, e)`), so the PR contributes zero graph data. Pre-consolidation the file still existed on the checked-out main, so it produced some (not necessarily faithful) data — that number was never a clean reproduction of PR #1529's own diff-to-symbol mapping either.

Two candidate explanations were checked directly against source and ruled out:

- **Node-type coverage**: the retired `graph::query::Node` enum (pre-#1544) had exactly three variants — `File`, `Function`, `Module` — and `render_subgraph_text`'s match arm rendered only `Function`. The pre-#1529 docstring claiming `Struct`/`Enum`/`Trait`/`Impl` nodes were also rendered did not match the code; PR #1529 corrected the docstring to reality. No behavior changed.
- **Seed derivation**: `derive_modified_symbols` (including its `SYMBOL_RE` regex fallback for brand-new declarations) is byte-identical on both sides of the #1544 consolidation boundary (introduced in #1445; untouched by #1544 or by #1538's later fix). Seeding logic did not regress.

One genuine, already-documented rendering difference remains (`crates/aptu-core/src/graph/mod.rs:13-15`): the new renderer emits duplicate-name symbols once per distinct node rather than deduplicating by name, which makes output larger, not smaller, in that case — so it cannot explain the overall volume reduction either.

**Re-verification on aptu-coder-core 0.32.4** (current main @ 7cec7f7, includes #1559): cold-run numbers reproduce the Table 2/3 post-consolidation figures within normal tokenizer jitter — #1529 16,661 chars (5,454 tokens), #1531 75,134 chars (21,381 tokens), #1532 81,865 chars (28,626 tokens), #1519 81,913 chars (21,642 tokens); cache file sizes byte-identical (12B / 6,585B / 480B / 10,460B). Confirms #1559's decode-path fix does not touch the cold-build path, as expected.

**Impact:** no code regression to fix. The graph feature's cold-run rendering is behaving as designed; the apparent volume loss is an artifact of re-auditing already-merged historical PRs against a later checkout. Future KG benchmarks that replay historical PRs should either use still-open PRs or check out each PR's own head SHA into `repo_path` before that PR's cold run, to get a faithful measurement.

### F14: R10 verified — #1559 restores cold/warm parity (VERIFICATION)

**Severity:** Info

**Category:** MEASUREMENT

Re-ran the KG config, 3 runs per PR (cold + 2 warm), on aptu-coder-core 0.32.4 (main @ e5c164a, includes #1559):

| PR | Cold (run 1) | Warm (run 2) | Warm (run 3) | Cold = Warm? |
|------|------|------|------|------|
| 1519 | 81,913 chars / 21,642 tokens | 81,913 / 21,642 | 81,913 / 21,642 | Yes |
| 1529 | 16,661 / 5,454 | 16,661 / 5,454 | 16,661 / 5,466 | Yes (0 injection either way, per F13) |
| 1531 | 75,134 / 21,381 | 75,134 / 21,381 | 75,134 / 21,381 | Yes |
| 1532 | 81,865 / 28,614 | 81,865 / 28,614 | 81,865 / 28,614 | Yes |

Before #1559, warm runs on these same PRs dropped to the No KG baseline (Table 2: e.g. #1531 warm was 72,442/20,605 vs cold 75,134/21,369). Now every warm run matches its cold run exactly. Duration showed no consistent cold-vs-warm pattern (e.g. #1532: cold 4,360ms, warm 3,868ms/2,971ms, faster; #1531: cold 2,987ms, warm 4,267ms/4,646ms, slower) — dominated by AI-provider response variance, not local cache lookup cost, consistent with F12.

**Impact:** R10's acceptance criterion (cold/warm injection identical) is met. F9/F10 are resolved; the graph feature's cache-hit path is release-verifiable again.

### F15: StructuralGraph is 6.6-33x faster than the retired local pipeline, and scales better (PERFORMANCE)

**Severity:** Info

**Category:** PERFORMANCE

Follow-up to the "is the new implementation faster or better" question. Local, AI-free microbenchmark (no network calls): synthetic fixtures of realistic Rust functions with cross-file call chains, timing only the graph pipeline (build + seed + blast-radius + render) with `std::time::Instant`, release profile, 200 iterations each.

| Fixture | OLD (`graph::builder` + `graph::query`, pre-#1544) | NEW (`StructuralGraph`) | Speedup |
|------|------|------|------|
| 5 files / 50 functions (typical PR size) | median 157.8µs | median 23.9µs | ~6.6x |
| 30 files / 300 functions (large PR size) | median 3,650.8µs | median 110.6µs | ~33x |

Scaling: 6x more files made OLD ~23x slower (worse than linear) but NEW only ~4.6x slower (sub-linear).

**Mechanism** (verified by inspecting the actual rendered output, not just timings): the retired `builder::build_from_analysis` is called once per file, but against a `CallGraph` built globally across all changed files. Each per-file call only recognizes its own functions; every cross-file callee it doesn't recognize gets a fresh placeholder node created on the spot, and the resulting per-file graphs are merged naively. With F files this creates duplicate nodes for the same symbol across files — slower to build and merge, and it also inflates the rendered output: the 30-file OLD run rendered 182 lines / 7,830 chars, but with the same line (e.g. `fn func_0_1 [calls: func_0_2] [callers: func_0_0]`) repeated multiple times from duplicate nodes matching by name. The NEW pipeline on the identical input rendered a clean 7 lines / 403 chars, no duplication.

**Impact:** answers the earlier open question — the retired implementation's larger character counts in the real historical benchmark (Table 3/4, F11) were not necessarily richer content; at least part of that volume was likely this duplicate-node artifact, not additional useful information. The consolidation (#1544) was a net improvement on both speed and, in this respect, output quality — not merely a maintainability tradeoff.

**Caveat:** absolute local pipeline time (tens to low hundreds of microseconds) is negligible next to the AI-provider round-trip that dominates `aptu pr review`'s wall-clock time (seconds, per Table 1). This is a real, reproducible, mechanistically-explained speedup, but it is not what limits current review latency.

### F16: Same-name collision resolution scales super-linearly, but only becomes visible far beyond real PR sizes (PERFORMANCE)

**Severity:** Info

**Category:** PERFORMANCE

Follow-up to R12. F15's fixtures used uniquely-named functions across files, so `StructuralGraph::build_from_analysis`'s documented same-file-preference / line-proximity / arg-count collision resolution (`resolve_candidate()`, `aptu-coder-core` 0.32.4 `src/graph/structural.rs:90-163`) was never exercised. This benchmark adds a paired "collision" fixture: every file defines the same 5 names (`new`, `default`, `fmt`, `from`, `get`) instead of unique names, with the same 7-functions-per-file shape and the same cross-file chain structure as the control fixture, so file count is the only variable. Same harness as F15 (`std::time::Instant`, release profile, 200 iterations, local/AI-free), extended to N = 10/30/50/100/200/400 files to see the full scaling curve, not just one point.

| Files | Control median (unique names) | Collision median (5 names, N-way) | Ratio |
|------|------|------|------|
| 10 | 23.96µs | 35.38µs | 1.48x |
| 30 | 57.42µs | 79.83µs | 1.39x |
| 50 | 96.04µs | 143.96µs | 1.50x |
| 100 | 187.58µs | 374.71µs | 2.00x |
| 200 | 374.21µs | 865.50µs | 2.31x |
| 400 | 744.33µs | 3,236.33µs | 4.35x |

**Mechanism:** `resolve_candidate()` only takes its O(1) path when a name has 0 or 1 global candidates (`structural.rs:98-103`). With a name repeated across N files, every call site referencing it has N candidates, and stage b (same-file preference, `structural.rs:106-116`) does a full linear scan over all N candidates before it can narrow the pool — even when the scan's outcome is an unambiguous same-file match. `build_from_analysis` runs this resolution once per caller and once per callee for every recorded call, unconditionally, during graph *build* (not gated by seed or depth), so the cost lands regardless of which symbol is later queried. Confirms the O(N) source read from R12 directly: the collision/control ratio does not stay flat, it climbs monotonically (1.4x -> 1.5x -> 2.0x -> 2.3x -> 4.35x) as N grows, which a fixed constant-factor overhead would not do. Control itself scales sub-linearly (~31x time for a 40x file-count increase, consistent with F15), while collision scales faster than control at every step, most sharply between 200 and 400 files (ratio nearly doubles in that one step). Render output size stayed flat across all N for both fixture types (control: 6 lines / 314-315 chars; collision: 27 lines / 1,253-1,257 chars) — the growth differential is entirely in the build phase, not rendering, isolating collision resolution as the sole driver.

**Impact:** the effect is real and matches R12's hypothesis, but the file counts where it becomes noticeable (100+) are well beyond what `build_ast_context_sync` (`crates/aptu-core/src/ast_context.rs:151-247`) normally sees: `entries` there is built strictly from `files: &[PrFile]`, the PR's own changed-file list, not the whole repo. Realistic PRs run tens of files, where the ratio (1.4-1.5x) is a few-microsecond difference, negligible next to the AI round-trip per F15's caveat. This is not a regression to fix urgently, but it is a genuine untested slow path: an unusually large PR (a repo-wide rename or trait-impl sweep touching hundreds of files that each define `new`/`default`/`fmt`) would hit measurably worse-than-control scaling, and the local pipeline cost, while still small in absolute terms at N=400 (3.2ms), is no longer negligible-by-inspection the way F15's uniquely-named fixtures suggested.

## Recommendations

### R8: Fix symbol-index deserialization upstream before relying on the graph cache

**Priority:** High

**Fixes:** F9, F10

`StructuralGraph` in `aptu-coder-core` must rebuild `symbol_index` after deserialization (serde post-deserialization hook or a public rebuild API; `rebuild_symbol_index()` already exists as `pub(crate)`). In aptu, extend the roundtrip test in `crates/aptu-core/src/graph/cache.rs` to assert `find_symbols_all` returns non-empty seeds after decode, not only node/edge counts.

### R9: Treat the graph feature as not release-verified for cache-hit usage

**Priority:** High

**Fixes:** F9, F10, F11

The graph feature is opt-in (`[graph] enabled = true`; default off), so default-configuration behavior is unaffected. However, the consolidated implementation regressed both documented invariants (F4: cache determinism; F6-equivalent: consistent injection), and PR #1529 now builds an empty graph where the retired builder did not. KG default-enablement remains blocked per baseline R6, and the consolidation's "preserving existing behavior" claim does not hold for the cache-hit path. Re-run this benchmark after R8 to verify the F4 invariant before any release that advertises the graph feature.

### R10: Re-run this benchmark after the upstream fix

**Priority:** Info

**Fixes:** F9

**Status:** Verified — see F14.

Same 4 PRs, same method. Acceptance: KG prompt chars identical across cold and warm runs per PR, and warm-run injection equal to cold-run injection.

### R11: Accept current cold-run volume; fix the replay methodology, not the code

**Priority:** Info

**Fixes:** F11, F13

No restoration-to-parity work is warranted: the retired implementation's higher pre-consolidation numbers were not a more faithful measurement, just differently drifted against a different checkout state. Closes #1557. When replaying historical merged PRs for future graph benchmarks, either use still-open PRs or check out each PR's own head SHA into `repo_path` before its cold run.

### R12: Benchmark same-name symbol collision resolution before assuming F15's speedup holds everywhere

**Priority:** Info

**Fixes:** F15

F15's fixtures used uniquely-named functions, so they never exercised `StructuralGraph::build_from_analysis`'s documented same-file-preference / line-proximity / arg-count collision resolution (`graph/mod.rs`'s doc comment, upstream since aptu-coder-core 0.32.0). Real Rust codebases commonly repeat names like `new`, `default`, `fmt`, `from` across many files/impls; if that resolution step is not O(1) or O(log n) per symbol, a repo heavy with such collisions could hit a slower path this benchmark never touched. Not yet measured — a fixture with many same-named functions across files would settle it.

**Status:** Measured — see F16.

### R13: Index collision candidates by file to remove the O(N) same-file scan, but treat it as low priority

**Priority:** Info

**Fixes:** F16

`resolve_candidate()`'s same-file-preference stage (`structural.rs:106-116`) filters the *entire* candidate list for a name on every call, even when the eventual match is unambiguous. Grouping `symbol_index` by `(name, file_path)` — or keeping a secondary `HashMap<(String, String), NodeIndex>` populated during `build_nodes` — would make same-file lookups O(1) and remove the dominant cost driver F16 measured, without changing `resolve_candidate`'s line-proximity/arg-count fallback stages (which only run on the rarer same-file-miss or same-line-tie paths). Not urgent: `build_ast_context_sync` bounds `entries` to a single PR's changed-file list, and F16 shows the effect costs low-single-digit milliseconds even at 400 colliding files — a PR size well outside normal review workloads. Worth doing opportunistically upstream in `aptu-coder-core`, not worth blocking on.

## Summary

*Table 5: Findings.*

| ID | Severity | Category | Finding |
|----|----------|----------|---------|
| F9 | High | BUG | Decoded StructuralGraph loses `symbol_index` (`#[serde(skip)]`); warm cache hits inject zero context |
| F10 | High | BUG | KG is a steady-state no-op on cache-hit reviews (8 of 12 KG runs) |
| F11 | Info | MEASUREMENT | Cold-run injection 21-95% below pre-consolidation baseline; #1529 builds an empty graph |
| F12 | Info | MEASUREMENT | No quality signal; input_tokens confirmed as primary metric |
| F13 | Info | MEASUREMENT | Cold-run volume gap is a replay-methodology artifact (checkout-time file drift, #1529's file deleted by #1544), not a StructuralGraph regression; 0.32.4 re-verified |
| F14 | Info | MEASUREMENT | Cold/warm parity confirmed restored on aptu-coder-core 0.32.4 (#1559); R10 acceptance criterion met |
| F15 | Info | PERFORMANCE | StructuralGraph is 6.6-33x faster than the retired local pipeline on synthetic fixtures, and scales sub-linearly where the retired pipeline scaled worse than linear; retired pipeline's larger output was partly duplicate-node noise |
| F16 | Info | PERFORMANCE | Same-name collision resolution scales super-linearly (ratio climbs 1.4x -> 4.35x from 10 to 400 colliding files); only becomes material well beyond real PR sizes (`entries` is bounded by PR changed-files) |

*Table 6: Recommendations.*

| ID | Priority | Fixes | Recommendation |
|----|----------|-------|----------------|
| R8 | High | F9, F10 | Rebuild symbol index post-deserialize upstream; extend aptu roundtrip test |
| R9 | High | F9-F11 | Graph feature not release-verified for cache-hit usage; re-verify before release |
| R10 | Info | F9 | Re-run benchmark post-fix; acceptance: cold/warm injection identical — **Verified, see F14** |
| R11 | Info | F11, F13 | Accept current cold-run volume; fix future replay methodology instead of code; closes #1557 |
| R12 | Info | F15 | Benchmark same-name symbol collision resolution — untested scaling path — **Measured, see F16** |
| R13 | Info | F16 | Index collision candidates by file to remove the O(N) same-file scan; low priority, not worth blocking on |

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
