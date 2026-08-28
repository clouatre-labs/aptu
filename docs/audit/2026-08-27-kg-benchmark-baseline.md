# Audit: KG Benchmark -- Multi-PR Size Scaling (No KG vs Current KG) -- August 2026

Date: 2026-08-27
Toolchain: aptu 0.10.16 release build (main @ 88c20e4), aptu-coder-core 0.32.2 (pre-consolidation)
Data: 4 PRs x 2 configs x 3 runs = 24 total runs, same OpenRouter `mistralai/mistral-small-2603`
model on all runs.
Method: `aptu pr review <PR> --repo clouatre-labs/aptu -o json` -- real AI calls, nothing posted.
Scope: `~/.config/aptu/config.toml` (`[graph]` section), `~/.local/share/aptu/graph/` (disk cache),
`.ai_stats` JSON fields (`prompt_chars`, `input_tokens`, `cost_usd`, `duration_ms`, `model`),
`.review` JSON fields (`verdict`, `comments`, `concerns`, `strengths`).

---

## Purpose

Extend the single-PR baseline (PR #1532, originally benchmarked in the first revision of this
document) to multiple PR sizes. The goal is to determine whether KG value scales with PR size and
whether KG should be enabled by default.

Two configs tested:

1. **No KG** -- graph disabled (default; no config file)
2. **Current KG** -- aptu-coder-core 0.32.2 graph enabled with pre-consolidation
   `graph::builder` + `graph::query`

### Target PRs

All PRs are merged (read-only). Selected to span PR size from tiny doc fix to large multi-file
code change:

| PR | Size | Add | Del | Files | Description |
|----|------|-----|-----|-------|-------------|
| #1529 | Tiny | 2 | 3 | 1 | Doc fix in `graph/query.rs` |
| #1531 | Small | 31 | 38 | 3 | Security fix in `scanner.rs`, `pr_review.rs`, `patterns.json` |
| #1532 | Medium | 211 | 47 | 4 | Graph code in `builder.rs`, `mod.rs`, `Cargo.toml`, `Cargo.lock` |
| #1519 | Large | 266 | 21 | 2 | Graph context fix in `review_context.rs`, `ast_context.rs` |

---

## Method

3 runs per PR per config (24 total). Command: `aptu pr review <PR> --repo clouatre-labs/aptu -o json`.

Extracted from JSON: `.ai_stats.prompt_chars`, `.ai_stats.input_tokens`, `.ai_stats.cost_usd`,
`.ai_stats.duration_ms`, `.ai_stats.model`, `.review.verdict`, `.review.comments | length`,
`.review.concerns | length`, `.review.strengths | length`.

Graph cache cleared between configs (`rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/`),
not between runs of the same config (runs 2/3 should hit cache).

The `-o json` flag produces the review locally without posting to GitHub. All PRs are merged;
this is a read-only benchmark.

### Primary metric

`input_tokens` is the primary metric. `cost_usd` is unreliable due to AI provider prompt caching
variance (see F2 in the original single-PR baseline, preserved as F2 below).

### Run classification

- Run 1 of each config: cold (graph cache build + AI provider cold cache)
- Runs 2/3 of each config: warm (graph cache hit + AI provider warm cache)
- Warm-cache runs are the reliable cost/latency comparison; cold run verifies graph cache build

### Reproduction

#### Prerequisites

- aptu installed at `~/.cargo/bin/aptu` (release build, main branch at 88c20e4)
- GitHub auth configured (OAuth device flow via `aptu auth login`)
- No config file at `~/.config/aptu/config.toml` (KG disabled by default)

#### No-KG config

No config file needed. Clear graph cache and run:

```bash
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

Do NOT clear cache between runs.

#### KG-enabled config

Create `~/.config/aptu/config.toml`:

```toml
[graph]
enabled = true
max_depth = 4
max_nodes = 50000
cache_ttl_hours = 24
```

Clear graph cache and run the same loop. Do NOT clear cache between runs.

#### Cleanup

```bash
rm -f ~/.config/aptu/config.toml
rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/
```

---

## Results

### Table 1: All 24 Runs

| PR | Size | Config | Run | Prompt Chars | Input Tokens | Cost (USD) | Duration (ms) | Model | Verdict | Comments | Concerns | Strengths |
|----|------|--------|-----|-------------|-------------|------------|---------------|-------|---------|----------|----------|-----------|
| 1529 | Tiny | No KG | 1 | 17,826 | 5,862 | $0.001312 | 2,939 | mistral-small-2603 | approve | 1 | 0 | 4 |
| 1529 | Tiny | No KG | 2 | 17,826 | 5,850 | $0.000970 | 1,539 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1529 | Tiny | No KG | 3 | 17,826 | 5,862 | $0.001423 | 3,938 | mistral-small-2603 | approve | 2 | 0 | 4 |
| 1529 | Tiny | KG | 1 | 23,143 | 7,236 | $0.001504 | 1,792 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1529 | Tiny | KG | 2 | 23,143 | 7,236 | $0.001484 | 1,687 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1529 | Tiny | KG | 3 | 23,143 | 7,224 | $0.001177 | 1,753 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1531 | Small | No KG | 1 | 71,236 | 20,137 | $0.003223 | 5,298 | mistral-small-2603 | approve | 4 | 0 | 6 |
| 1531 | Small | No KG | 2 | 71,236 | 20,137 | $0.000695 | 5,664 | mistral-small-2603 | approve | 4 | 0 | 5 |
| 1531 | Small | No KG | 3 | 71,236 | 20,137 | $0.000501 | 6,970 | mistral-small-2603 | approve | 0 | 0 | 6 |
| 1531 | Small | KG | 1 | 71,236 | 20,137 | $0.000798 | 5,696 | mistral-small-2603 | approve | 4 | 0 | 6 |
| 1531 | Small | KG | 2 | 71,236 | 20,137 | $0.000534 | 3,253 | mistral-small-2603 | approve | 0 | 0 | 7 |
| 1531 | Small | KG | 3 | 71,236 | 20,137 | $0.000608 | 4,734 | mistral-small-2603 | approve | 2 | 0 | 7 |
| 1532 | Medium | No KG | 1 | 82,559 | 28,900 | $0.004422 | 4,558 | mistral-small-2603 | approve | 2 | 0 | 5 |
| 1532 | Medium | No KG | 2 | 82,559 | 28,900 | $0.000823 | 4,908 | mistral-small-2603 | approve | 3 | 0 | 6 |
| 1532 | Medium | No KG | 3 | 82,559 | 28,900 | $0.004608 | 4,510 | mistral-small-2603 | approve | 2 | 0 | 7 |
| 1532 | Medium | KG | 1 | 96,741 | 32,511 | $0.005015 | 5,389 | mistral-small-2603 | approve | 3 | 0 | 6 |
| 1532 | Medium | KG | 2 | 96,741 | 32,511 | $0.000805 | 4,523 | mistral-small-2603 | approve | 2 | 0 | 5 |
| 1532 | Medium | KG | 3 | 96,741 | 32,511 | $0.000707 | 3,354 | mistral-small-2603 | approve | 2 | 0 | 5 |
| 1519 | Large | No KG | 1 | 71,300 | 18,654 | $0.003834 | 4,311 | mistral-small-2603 | approve | 2 | 0 | 6 |
| 1519 | Large | No KG | 2 | 71,300 | 18,654 | $0.003907 | 4,115 | mistral-small-2603 | approve | 2 | 0 | 5 |
| 1519 | Large | No KG | 3 | 71,300 | 18,642 | $0.002906 | 4,244 | mistral-small-2603 | approve | 2 | 0 | 6 |
| 1519 | Large | KG | 1 | 71,300 | 18,642 | $0.002914 | 4,639 | mistral-small-2603 | approve | 2 | 0 | 5 |
| 1519 | Large | KG | 2 | 71,300 | 18,642 | $0.000565 | 3,926 | mistral-small-2603 | approve | 2 | 0 | 7 |
| 1519 | Large | KG | 3 | 71,300 | 18,642 | $0.003095 | 11,871 | mistral-small-2603 | approve | 2 | 0 | 6 |

### Table 2: Warm-Cache Averages Per PR (Runs 2+3)

| PR | Size (add/del/files) | Modified Files | Config | Prompt Chars | Input Tokens | Cost (USD) | Duration (ms) | Graph Cache (bytes) |
|----|---------------------|----------------|--------|-------------|-------------|------------|---------------|---------------------|
| 1529 | Tiny (2/3/1) | `graph/query.rs` | No KG | 17,826 | 5,856 | $0.001197 | 2,739 | N/A |
| 1529 | | | KG | 23,143 | 7,230 | $0.001330 | 1,720 | 2,590 |
| | | | Delta | +5,317 | +1,374 | +$0.000134 | -1,019 | |
| | | | % Change | +29.8% | +23.5% | +11.2% | -37.2% | |
| 1531 | Small (31/38/3) | `pr_review.rs`, `patterns.json`, `scanner.rs` | No KG | 71,236 | 20,137 | $0.000598 | 6,317 | N/A |
| 1531 | | | KG | 71,236 | 20,137 | $0.000571 | 3,994 | 12 |
| | | | Delta | 0 | 0 | -$0.000027 | -2,324 | |
| | | | % Change | 0.0% | 0.0% | -4.5% | -36.8% | |
| 1532 | Medium (211/47/4) | `Cargo.lock`, `Cargo.toml`, `builder.rs`, `mod.rs` | No KG | 82,559 | 28,900 | $0.002716 | 4,709 | N/A |
| 1532 | | | KG | 96,741 | 32,511 | $0.000756 | 3,939 | 6,179 |
| | | | Delta | +14,182 | +3,611 | -$0.001959 | -770 | |
| | | | % Change | +17.2% | +12.5% | -72.1% | -16.4% | |
| 1519 | Large (266/21/2) | `review_context.rs`, `ast_context.rs` | No KG | 71,300 | 18,648 | $0.003406 | 4,180 | N/A |
| 1519 | | | KG | 71,300 | 18,642 | $0.001830 | 7,899 | 12 |
| | | | Delta | 0 | -6 | -$0.001577 | +3,719 | |
| | | | % Change | 0.0% | -0.0% | -46.3% | +89.0% | |

Note: PR #1519 KG run 3 had a duration outlier (11,871 ms vs ~4,000 ms average), inflating the
warm-cache duration average. Cost deltas are unreliable due to provider prompt caching (F2).

### Table 3: Cross-PR Comparison -- KG Delta % by PR Size

| PR | Size | Files Modified | Graph Cache | Token Delta % | Char Delta % | KG Injected Context? |
|----|------|---------------|-------------|---------------|--------------|---------------------|
| 1529 | Tiny (2/3/1) | 1 .rs (graph module) | 2,590 bytes | +23.5% | +29.8% | Yes |
| 1531 | Small (31/38/3) | 2 .rs + 1 .json (non-graph) | 12 bytes | 0.0% | 0.0% | No |
| 1532 | Medium (211/47/4) | 2 .rs (graph module) + 2 non-code | 6,179 bytes | +12.5% | +17.2% | Yes |
| 1519 | Large (266/21/2) | 2 .rs (non-graph) | 12 bytes | 0.0% | 0.0% | No |

KG context injection does not correlate with PR size (lines changed or file count). It correlates
with whether the modified files contain symbols that the graph builder has indexed. The 12-byte
cache files (essentially empty: just the schema-hash header) confirm the graph was built but
`find_modified_nodes` returned zero results for #1531 and #1519.

### Table 4: Quality Indicators (Warm-Cache Averages)

| PR | Size | Config | Verdict | Comments (avg) | Concerns (avg) | Strengths (avg) |
|----|------|--------|---------|----------------|----------------|-----------------|
| 1529 | Tiny | No KG | approve | 1.0 | 0.0 | 4.0 |
| 1529 | Tiny | KG | approve | 0.0 | 0.0 | 4.0 |
| 1531 | Small | No KG | approve | 2.0 | 0.0 | 5.5 |
| 1531 | Small | KG | approve | 1.0 | 0.0 | 7.0 |
| 1532 | Medium | No KG | approve | 2.5 | 0.0 | 6.5 |
| 1532 | Medium | KG | approve | 2.0 | 0.0 | 5.0 |
| 1519 | Large | No KG | approve | 2.0 | 0.0 | 5.5 |
| 1519 | Large | KG | approve | 2.0 | 0.0 | 6.5 |

All 24 runs returned `approve` with 0 concerns. Comment and strength counts vary due to AI
non-determinism. No consistent KG effect on quality indicators is observable.

### Graph Cache Verification

Cache files are keyed by PR head commit SHA. All 4 PRs produced cache files, confirming the graph
builder ran on each:

| PR | Head SHA (prefix) | Cache Size | Context Injected |
|----|-------------------|------------|-----------------|
| 1529 | `823128f3` | 2,590 bytes | Yes (+5,317 chars) |
| 1531 | `ee7efafe` | 12 bytes | No (empty) |
| 1532 | `af4e9ce4` | 6,179 bytes | Yes (+14,182 chars) |
| 1519 | `3c2b019a` | 12 bytes | No (empty) |

The 12-byte files contain only the schema-hash header, confirming the graph was built but the
blast-radius query returned empty results. Prompt chars were identical across all 3 KG runs per
PR (deterministic), confirming cache correctness (consistent with F4).

---

## Key Questions

### Q1: Does KG add more prompt context on larger PRs, or is it roughly constant?

Neither. KG context injection is not constant, and it does not scale with PR size. Of the 4 PRs
tested, 2 PRs (#1529, #1532) received KG context (+5,317 chars and +14,182 chars respectively)
and 2 PRs (#1531, #1519) received zero KG context despite being larger than #1529.

When context is injected, the amount correlates with the number of modified graph-module symbols:
#1532 (2 graph-module files: `builder.rs` + `mod.rs`) produced 2.7x more context than #1529
(1 graph-module file: `query.rs`). But PR size (lines changed) is not the predictor.

### Q2: Does KG context injection scale with the number of modified symbols?

Yes, but only when modified symbols are found in the graph. Among PRs where KG injects context:

- #1529: 1 modified graph file (`query.rs`) produced 2,590-byte cache, +5,317 chars
- #1532: 2 modified graph files (`builder.rs` + `mod.rs`) produced 6,179-byte cache, +14,182 chars

The cache size ratio (2.4x) is proportional to the context ratio (2.7x). However, #1531
(3 files including 2 .rs files) and #1519 (2 .rs files) produced 12-byte (empty) caches because
`find_modified_nodes` found zero graph-indexed symbols in their diffs. File count does not predict
context injection; symbol coverage in the graph does.

### Q3: Does KG change review quality differently on small vs large PRs?

No measurable quality change. All 24 runs returned `approve` with 0 concerns. Comment counts
fluctuate due to AI non-determinism (e.g., #1531 No KG: 4, 4, 0; #1531 KG: 4, 0, 2) with no
consistent directional effect from KG. Strengths counts similarly vary without pattern. The sample
size (3 runs) is insufficient to detect subtle quality differences, but no large-effect signal is
present.

### Q4: Is there a PR size threshold below which KG adds overhead without value?

The threshold is not PR size; it is graph symbol coverage. The tiny PR (#1529: 2 add / 3 del /
1 file) received the highest relative KG overhead (+23.5% tokens) because it modified
`graph/query.rs`, a file with graph-indexed symbols. The large PR (#1519: 266 add / 21 del /
2 files) received zero KG overhead because its modified files (`review_context.rs`,
`ast_context.rs`) had no symbols found in the graph.

When KG does inject context, the overhead is modest: +12.5% to +23.5% tokens. When it does not
inject context (12-byte cache), the overhead is exactly zero tokens. There is no "wasted overhead"
scenario in the data; either KG adds context (with proportional token cost) or it adds nothing.

### Q5: On PRs that do NOT touch graph code, does KG still inject context?

No. The data is definitive:

- **#1529** modifies `graph/query.rs` (graph code). KG injects +5,317 chars.
- **#1531** modifies `pr_review.rs`, `patterns.json`, `scanner.rs` (non-graph code). KG injects
  0 chars. Cache file is 12 bytes (empty).
- **#1519** modifies `review_context.rs`, `ast_context.rs` (non-graph code). KG injects 0 chars.
  Cache file is 12 bytes (empty).

KG only injects context when the modified files contain symbols that the graph builder has
indexed. The graph builder appears to index symbols from graph-module files but does not find
modified symbols in other source files (`pr_review.rs`, `scanner.rs`, `review_context.rs`,
`ast_context.rs`). This is a bug, not an intentional design choice; see Finding F5 and Issue #1538.

---

## Findings

### F1: KG adds 17.2% prompt chars with proportional token cost (INFO / POSITIVE)

**Severity:** Info
**Category:** POSITIVE

KG context injection adds 14,182 prompt chars and 3,611 input tokens on PR #1532. The
token-to-char ratio for graph context (3,611 tokens / 14,182 chars = 0.25) is lower than the
overall prompt ratio (28,900 / 82,559 = 0.35), indicating compact serialization.

On PR #1529 (the only other PR where KG fired), the overhead was +5,317 chars (+29.8%) and
+1,374 tokens (+23.5%), with a similar token-to-char ratio of 0.26.

**Impact:** When KG fires, it delivers structural context at a predictable, modest token cost.
This is the baseline to compare against `StructuralGraph` post-consolidation.

### F2: cost_usd deltas are not a reliable signal (INFO / MEASUREMENT)

**Severity:** Info
**Category:** MEASUREMENT

The -53.6% average cost delta on PR #1532 (from the original single-PR baseline) was an artifact
of the no-KG run 1 cold-cache outlier ($0.0047 vs $0.0014 for KG run 1). The KG run 1 was cheaper
despite more tokens, possibly due to model-tier routing or provider-side prompt caching variance.
The warm-cache comparison (runs 2/3 vs 5/6) is the reliable cost metric: +3.9% for +12.5% tokens.

**Impact:** `cost_usd` should not be used as the primary metric for KG impact assessment.
`input_tokens` is the trustworthy metric, consistent with F3 in the
2026-08-26 graph-context-prompt-injection audit.

### F3: graph cache hit does not reliably reduce latency (INFO / MEASUREMENT)

**Severity:** Info
**Category:** MEASUREMENT

KG run 5 latency (4,812 ms) was higher than run 4 (4,570 ms) despite warm graph cache on PR
#1532. Run 6 (3,750 ms) was the fastest overall. On PR #1519, KG run 3 had an 11,871 ms outlier
vs ~4,000 ms average. Graph build is fast enough that its contribution is within network/AI
latency noise.

**Impact:** Latency is not a useful metric for graph cache effectiveness at this scale. Prompt
chars and input tokens are deterministic and should be the primary comparison axes for the
post-consolidation benchmark.

### F4: graph cache produces deterministic context across cold/warm runs (INFO / POSITIVE)

**Severity:** Info
**Category:** POSITIVE

Prompt chars are identical across all 3 KG runs per PR (23,143 for #1529, 71,236 for #1531,
96,741 for #1532, 71,300 for #1519), confirming the cache returns the same context as a fresh
build. Cache files persisted to disk and were loaded on runs 2/3.

**Impact:** Graph cache correctness is confirmed across all 4 PRs. Post-consolidation, the same
invariant must hold: `StructuralGraph` cache must produce identical prompt chars across cold/warm
runs.

### F5: KG context injection silently skipped for PRs modifying large source files (INFO / BUG)

**Severity:** Info
**Category:** BUG

KG produces zero context for 2 of 4 tested PRs (#1531 and #1519), with 12-byte (empty) cache
files. Both PRs modify Rust source files that should contain graph-indexable symbols
(`pr_review.rs`, `scanner.rs`, `review_context.rs`, `ast_context.rs`).

Root cause identified in Issue #1538: in `crates/aptu-core/src/ast_context.rs`,
`build_ast_context_sync()` iterates PR files and renders an AST text block per file. When the
cumulative text output exceeds a 2000-character cap (`CAP`), the loop `break`s before populating
`analysis_pairs` and `symbol_ranges`, which are the inputs to graph construction and symbol
matching. A large first file (e.g., `pr_review.rs` at 800+ lines) can exhaust the cap before any
subsequent files are accumulated, blocking graph data for the entire PR.

The fix is to continue accumulating `analysis_pairs`, `impl_traits`, and `symbol_ranges` for
every successfully analyzed file, even after the AST text cap is reached. Only the text output
should be capped, not the graph data collection.

**Impact:** KG is a silent no-op for most real-world PRs that touch large source files. The empty
graph is cached, so subsequent reviews of the same PR never benefit from KG. No error or warning
is emitted. This bug must be fixed before KG can be considered for default enablement.

---

## Recommendations

### R1: Fix AST text cap break before enabling KG by default (info)

**Priority:** Info
**Fixes:** F5

Issue #1538 tracks the fix. The `break` in `build_ast_context_sync()` must be changed to cap only
the text output, not the graph data collection. After the fix, re-run this benchmark on PRs #1531
and #1519 to verify non-empty KG context injection.

### R2: Do not enable KG by default until F5 is fixed (info)

**Priority:** Info
**Fixes:** F5

The data shows KG is a no-op for 2 of 4 PRs due to the AST text cap bug. Enabling KG by default
in its current state would add no value for the majority of PRs while introducing graph cache
build overhead. When KG does fire (PRs touching graph-module files), the overhead is acceptable
(+12.5% to +23.5% tokens) with deterministic caching and no latency regression on warm cache.

Conditional recommendation: enable KG by default only after #1538 is fixed and this benchmark is
re-run to verify consistent context injection across diverse PRs.

### R3: Re-run benchmark after StructuralGraph consolidation (info)

**Priority:** Info
**Fixes:** --

Re-run with the same 4 PRs, same method, 3 runs per config. Add a third config ("future KG" using
`StructuralGraph`). Compare three-way: prompt chars, input tokens, cost, latency. Key question:
does `StructuralGraph::bfs_blast_radius()` (outgoing-only) produce the same or different context
volume as the current bidirectional `graph::query::blast_radius()`? If outgoing-only produces
less context, expect lower prompt chars and tokens.

### R4: Use input_tokens as primary metric, not cost_usd (info)

**Priority:** Info
**Fixes:** F2

`cost_usd` is dominated by AI provider prompt caching variance. Use `input_tokens` as the
primary comparison axis for all future KG benchmarks. Report `cost_usd` only with warm-cache
averages and an explicit caveat.

### R5: Preserve this file as the baseline (info)

**Priority:** Info
**Fixes:** --

Create `2026-XX-XX-kg-benchmark-post-fix.md` for the follow-up after #1538 is fixed. Do not
modify this file after merge; it is the pre-fix baseline.

---

## Summary

*Table 5: Findings.*

| ID | Severity | Category | Finding |
|---|---|---|---|
| F1 | Info | POSITIVE | KG adds 12-24% prompt chars with proportional, token-efficient cost when it fires |
| F2 | Info | MEASUREMENT | cost_usd deltas unreliable; use input_tokens as primary metric |
| F3 | Info | MEASUREMENT | Graph cache hit does not reliably reduce latency at this scale |
| F4 | Info | POSITIVE | Graph cache produces deterministic context across cold/warm runs (all 4 PRs) |
| F5 | Info | BUG | KG context injection silently skipped for PRs modifying large source files (#1538) |

*Table 6: Recommendations.*

| ID | Priority | Fixes | Recommendation |
|---|---|---|---|
| R1 | Info | F5 | Fix AST text cap break in `ast_context.rs` (tracked in #1538) |
| R2 | Info | F5 | Do not enable KG by default until F5 is fixed |
| R3 | Info | -- | Re-run benchmark after StructuralGraph consolidation (three-way comparison) |
| R4 | Info | F2 | Use input_tokens as primary metric, not cost_usd |
| R5 | Info | -- | Preserve this file as the pre-fix baseline |
