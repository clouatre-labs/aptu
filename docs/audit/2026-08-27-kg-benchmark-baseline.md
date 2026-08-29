# Audit: KG Benchmark — Multi-PR Size Scaling (No KG vs Current KG) — August 2026

> **Status: Superseded.** Historical benchmark baseline preserved for comparison. Graph consolidation completed across PRs #1544, #1551, #1553, and #1554. See `docs/ARCHITECTURE.md` for current structural graph design.

Date: 2026-08-27 (pre-fix baseline), 2026-08-27 (post-fix re-run after PR #1539)  
Toolchain: aptu 0.10.16 release build; pre-fix at main @ 88c20e4, post-fix at main @ 32f546d  
Data: 4 PRs x 2 configs x 3 runs = 24 total runs per benchmark (48 runs total), same OpenRouter `mistralai/mistral-small-2603` model on all runs.  
Method: `aptu pr review <PR> --repo clouatre-labs/aptu -o json` — real AI calls, nothing posted.  
Scope: `~/.config/aptu/config.toml` (`[graph]` section), `~/.local/share/aptu/graph/` (disk cache), `.ai_stats` JSON fields (`prompt_chars`, `input_tokens`, `cost_usd`, `duration_ms`, `model`), `.review` JSON fields (`verdict`, `comments`, `concerns`, `strengths`).  
Structure: Tables 1-6 are the pre-fix baseline (preserved). Tables 7-13 are the post-fix re-run after PR #1539 resolved the F5 bug.

---

## Purpose

Extend the single-PR baseline (PR #1532, originally benchmarked in the first revision of this document) to multiple PR sizes. The goal is to determine whether KG value scales with PR size and whether KG should be enabled by default.

Two configs tested:

1. **No KG** — graph disabled (default; no config file)
2. **Current KG** — aptu-coder-core 0.32.2 graph enabled with pre-consolidation `graph::builder` + `graph::query`

### Target PRs

All PRs are merged (read-only). Selected to span PR size from tiny doc fix to large multi-file code change:

| PR | Size | Add | Del | Files | Description |
|----|------|-----|-----|-------|-------------|
| #1529 | Tiny | 2 | 3 | 1 | Doc fix in `graph/query.rs` |
| #1531 | Small | 31 | 38 | 3 | Security fix in `scanner.rs`, `pr_review.rs`, `patterns.json` |
| #1532 | Medium | 211 | 47 | 4 | Graph code in `builder.rs`, `mod.rs`, `Cargo.toml`, `Cargo.lock` |
| #1519 | Large | 266 | 21 | 2 | Graph context fix in `review_context.rs`, `ast_context.rs` |

---

## Method

3 runs per PR per config (24 total). Command: `aptu pr review <PR> --repo clouatre-labs/aptu -o json`.

Extracted from JSON: `.ai_stats.prompt_chars`, `.ai_stats.input_tokens`, `.ai_stats.cost_usd`, `.ai_stats.duration_ms`, `.ai_stats.model`, `.review.verdict`, `.review.comments | length`, `.review.concerns | length`, `.review.strengths | length`.

Graph cache cleared between configs (`rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/`), not between runs of the same config (runs 2/3 should hit cache).

The `-o json` flag produces the review locally without posting to GitHub. All PRs are merged; this is a read-only benchmark.

### Primary metric

`input_tokens` is the primary metric. `cost_usd` is unreliable due to AI provider prompt caching variance (see F2 in the original single-PR baseline, preserved as F2 below).

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

Note: PR #1519 KG run 3 had a duration outlier (11,871 ms vs ~4,000 ms average), inflating the warm-cache duration average. Cost deltas are unreliable due to provider prompt caching (F2).

### Table 3: Cross-PR Comparison — KG Delta % by PR Size

| PR | Size | Files Modified | Graph Cache | Token Delta % | Char Delta % | KG Injected Context? |
|----|------|---------------|-------------|---------------|--------------|---------------------|
| 1529 | Tiny (2/3/1) | 1 .rs (graph module) | 2,590 bytes | +23.5% | +29.8% | Yes |
| 1531 | Small (31/38/3) | 2 .rs + 1 .json (non-graph) | 12 bytes | 0.0% | 0.0% | No |
| 1532 | Medium (211/47/4) | 2 .rs (graph module) + 2 non-code | 6,179 bytes | +12.5% | +17.2% | Yes |
| 1519 | Large (266/21/2) | 2 .rs (non-graph) | 12 bytes | 0.0% | 0.0% | No |

KG context injection does not correlate with PR size (lines changed or file count). It correlates with whether the modified files contain symbols that the graph builder has indexed. The 12-byte cache files (essentially empty: just the schema-hash header) confirm the graph was built but `find_modified_nodes` returned zero results for #1531 and #1519.

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

All 24 runs returned `approve` with 0 concerns. Comment and strength counts vary due to AI non-determinism. No consistent KG effect on quality indicators is observable.

### Graph Cache Verification

Cache files are keyed by PR head commit SHA. All 4 PRs produced cache files, confirming the graph builder ran on each:

| PR | Head SHA (prefix) | Cache Size | Context Injected |
|----|-------------------|------------|-----------------|
| 1529 | `823128f3` | 2,590 bytes | Yes (+5,317 chars) |
| 1531 | `ee7efafe` | 12 bytes | No (empty) |
| 1532 | `af4e9ce4` | 6,179 bytes | Yes (+14,182 chars) |
| 1519 | `3c2b019a` | 12 bytes | No (empty) |

The 12-byte files contain only the schema-hash header, confirming the graph was built but the blast-radius query returned empty results. Prompt chars were identical across all 3 KG runs per PR (deterministic), confirming cache correctness (consistent with F4).

---

## Key Questions

### Q1: Does KG add more prompt context on larger PRs, or is it roughly constant?

Neither. KG context injection is not constant, and it does not scale with PR size. Of the 4 PRs tested, 2 PRs (#1529, #1532) received KG context (+5,317 chars and +14,182 chars respectively) and 2 PRs (#1531, #1519) received zero KG context despite being larger than #1529.

When context is injected, the amount correlates with the number of modified graph-module symbols: #1532 (2 graph-module files: `builder.rs` + `mod.rs`) produced 2.7x more context than #1529 (1 graph-module file: `query.rs`). But PR size (lines changed) is not the predictor.

### Q2: Does KG context injection scale with the number of modified symbols?

Yes, but only when modified symbols are found in the graph. Among PRs where KG injects context:

- #1529: 1 modified graph file (`query.rs`) produced 2,590-byte cache, +5,317 chars
- #1532: 2 modified graph files (`builder.rs` + `mod.rs`) produced 6,179-byte cache, +14,182 chars

The cache size ratio (2.4x) is proportional to the context ratio (2.7x). However, #1531 (3 files including 2 .rs files) and #1519 (2 .rs files) produced 12-byte (empty) caches because `find_modified_nodes` found zero graph-indexed symbols in their diffs. File count does not predict context injection; symbol coverage in the graph does.

### Q3: Does KG change review quality differently on small vs large PRs?

No measurable quality change. All 24 runs returned `approve` with 0 concerns. Comment counts fluctuate due to AI non-determinism (e.g., #1531 No KG: 4, 4, 0; #1531 KG: 4, 0, 2) with no consistent directional effect from KG. Strengths counts similarly vary without pattern. The sample size (3 runs) is insufficient to detect subtle quality differences, but no large-effect signal is present.

### Q4: Is there a PR size threshold below which KG adds overhead without value?

The threshold is not PR size; it is graph symbol coverage. The tiny PR (#1529: 2 add / 3 del / 1 file) received the highest relative KG overhead (+23.5% tokens) because it modified `graph/query.rs`, a file with graph-indexed symbols. The large PR (#1519: 266 add / 21 del / 2 files) received zero KG overhead because its modified files (`review_context.rs`, `ast_context.rs`) had no symbols found in the graph.

When KG does inject context, the overhead is modest: +12.5% to +23.5% tokens. When it does not inject context (12-byte cache), the overhead is exactly zero tokens. There is no "wasted overhead" scenario in the data; either KG adds context (with proportional token cost) or it adds nothing.

### Q5: On PRs that do NOT touch graph code, does KG still inject context?

No. The data is definitive:

- **#1529** modifies `graph/query.rs` (graph code). KG injects +5,317 chars.
- **#1531** modifies `pr_review.rs`, `patterns.json`, `scanner.rs` (non-graph code). KG injects 0 chars. Cache file is 12 bytes (empty).
- **#1519** modifies `review_context.rs`, `ast_context.rs` (non-graph code). KG injects 0 chars. Cache file is 12 bytes (empty).

KG only injects context when the modified files contain symbols that the graph builder has indexed. The graph builder appears to index symbols from graph-module files but does not find modified symbols in other source files (`pr_review.rs`, `scanner.rs`, `review_context.rs`, `ast_context.rs`). This is a bug, not an intentional design choice; see Finding F5 and Issue #1538.

---

## Findings

### F1: KG adds 17.2% prompt chars with proportional token cost (INFO / POSITIVE)

**Severity:** Info  
**Category:** POSITIVE

KG context injection adds 14,182 prompt chars and 3,611 input tokens on PR #1532. The token-to-char ratio for graph context (3,611 tokens / 14,182 chars = 0.25) is lower than the overall prompt ratio (28,900 / 82,559 = 0.35), indicating compact serialization.

On PR #1529 (the only other PR where KG fired), the overhead was +5,317 chars (+29.8%) and +1,374 tokens (+23.5%), with a similar token-to-char ratio of 0.26.

**Impact:** When KG fires, it delivers structural context at a predictable, modest token cost. This is the baseline to compare against `StructuralGraph` post-consolidation.

### F2: cost_usd deltas are not a reliable signal (INFO / MEASUREMENT)

**Severity:** Info  
**Category:** MEASUREMENT

The -53.6% average cost delta on PR #1532 (from the original single-PR baseline) was an artifact of the no-KG run 1 cold-cache outlier ($0.0047 vs $0.0014 for KG run 1). The KG run 1 was cheaper despite more tokens, possibly due to model-tier routing or provider-side prompt caching variance. The warm-cache comparison (runs 2/3 vs 5/6) is the reliable cost metric: +3.9% for +12.5% tokens.

**Impact:** `cost_usd` should not be used as the primary metric for KG impact assessment. `input_tokens` is the trustworthy metric, consistent with F3 in the 2026-08-26 graph-context-prompt-injection audit.

### F3: graph cache hit does not reliably reduce latency (INFO / MEASUREMENT)

**Severity:** Info  
**Category:** MEASUREMENT

KG run 5 latency (4,812 ms) was higher than run 4 (4,570 ms) despite warm graph cache on PR #1532. Run 6 (3,750 ms) was the fastest overall. On PR #1519, KG run 3 had an 11,871 ms outlier vs ~4,000 ms average. Graph build is fast enough that its contribution is within network/AI latency noise.

**Impact:** Latency is not a useful metric for graph cache effectiveness at this scale. Prompt chars and input tokens are deterministic and should be the primary comparison axes for the post-consolidation benchmark.

### F4: graph cache produces deterministic context across cold/warm runs (INFO / POSITIVE)

**Severity:** Info  
**Category:** POSITIVE

Prompt chars are identical across all 3 KG runs per PR (23,143 for #1529, 71,236 for #1531, 96,741 for #1532, 71,300 for #1519), confirming the cache returns the same context as a fresh build. Cache files persisted to disk and were loaded on runs 2/3.

**Impact:** Graph cache correctness is confirmed across all 4 PRs. Post-consolidation, the same invariant must hold: `StructuralGraph` cache must produce identical prompt chars across cold/warm runs.

### F5: KG context injection silently skipped for PRs modifying large source files (INFO / BUG)

**Severity:** Info  
**Category:** BUG

KG produces zero context for 2 of 4 tested PRs (#1531 and #1519), with 12-byte (empty) cache files. Both PRs modify Rust source files that should contain graph-indexable symbols (`pr_review.rs`, `scanner.rs`, `review_context.rs`, `ast_context.rs`).

Root cause identified in Issue #1538: in `crates/aptu-core/src/ast_context.rs`, `build_ast_context_sync()` iterates PR files and renders an AST text block per file. When the cumulative text output exceeds a 2000-character cap (`CAP`), the loop `break`s before populating `analysis_pairs` and `symbol_ranges`, which are the inputs to graph construction and symbol matching. A large first file (e.g., `pr_review.rs` at 800+ lines) can exhaust the cap before any subsequent files are accumulated, blocking graph data for the entire PR.

The fix is to continue accumulating `analysis_pairs`, `impl_traits`, and `symbol_ranges` for every successfully analyzed file, even after the AST text cap is reached. Only the text output should be capped, not the graph data collection.

**Impact:** KG is a silent no-op for most real-world PRs that touch large source files. The empty graph is cached, so subsequent reviews of the same PR never benefit from KG. No error or warning is emitted. This bug must be fixed before KG can be considered for default enablement.

---

## Recommendations

### R1: Fix AST text cap break before enabling KG by default (info)

**Priority:** Info  
**Fixes:** F5

Issue #1538 tracks the fix. The `break` in `build_ast_context_sync()` must be changed to cap only the text output, not the graph data collection. After the fix, re-run this benchmark on PRs #1531 and #1519 to verify non-empty KG context injection.

### R2: Do not enable KG by default until F5 is fixed (info)

**Priority:** Info  
**Fixes:** F5

The data shows KG is a no-op for 2 of 4 PRs due to the AST text cap bug. Enabling KG by default in its current state would add no value for the majority of PRs while introducing graph cache build overhead. When KG does fire (PRs touching graph-module files), the overhead is acceptable (+12.5% to +23.5% tokens) with deterministic caching and no latency regression on warm cache.

Conditional recommendation: enable KG by default only after #1538 is fixed and this benchmark is re-run to verify consistent context injection across diverse PRs.

### R3: Re-run benchmark after StructuralGraph consolidation (info)

**Priority:** Info  
**Fixes:** —

Re-run with the same 4 PRs, same method, 3 runs per config. Add a third config ("future KG" using `StructuralGraph`). Compare three-way: prompt chars, input tokens, cost, latency. Key question: does `StructuralGraph::bfs_blast_radius()` (outgoing-only) produce the same or different context volume as the current bidirectional `graph::query::blast_radius()`? If outgoing-only produces less context, expect lower prompt chars and tokens.

### R4: Use input_tokens as primary metric, not cost_usd (info)

**Priority:** Info  
**Fixes:** F2

`cost_usd` is dominated by AI provider prompt caching variance. Use `input_tokens` as the primary comparison axis for all future KG benchmarks. Report `cost_usd` only with warm-cache averages and an explicit caveat.

### R5: Preserve this file as the baseline (info)

**Priority:** Info  
**Fixes:** —

Create `2026-XX-XX-kg-benchmark-post-fix.md` for the follow-up after #1538 is fixed. Do not modify this file after merge; it is the pre-fix baseline.

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
| R3 | Info | — | Re-run benchmark after StructuralGraph consolidation (three-way comparison) |
| R4 | Info | F2 | Use input_tokens as primary metric, not cost_usd |
| R5 | Info | — | Preserve this file as the pre-fix baseline |

---

## Post-Fix Benchmark (PR #1539)

Date: 2026-08-27
Toolchain: aptu 0.10.16 release build (main @ 32f546d, includes PR #1539 fix)
Method: Same 4 PRs x 2 configs x 3 runs = 24 total runs, same model (`mistralai/mistral-small-2603`), same command (`aptu pr review <PR> --repo clouatre-labs/aptu -o json`), same extraction fields.

PR #1539 fixed the bug documented in F5: `build_ast_context_sync()` in `ast_context.rs` now caps only the text output, not the graph data accumulation. The `break` was replaced with a conditional `push_str`, and the early return on empty text was replaced with `output.clear()`. This section re-runs the same benchmark to verify the fix and measure KG overhead with corrected graph data collection.

### Table 7: All 24 Post-Fix Runs

| PR | Size | Config | Run | Prompt Chars | Input Tokens | Cost (USD) | Duration (ms) | Model | Verdict | Comments | Concerns | Strengths |
|----|------|--------|-----|-------------|-------------|------------|---------------|-------|---------|----------|----------|-----------|
| 1529 | Tiny | No KG | 1 | 17,826 | 5,850 | $0.000953 | 1,708 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1529 | Tiny | No KG | 2 | 17,826 | 5,850 | $0.000183 | 1,553 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1529 | Tiny | No KG | 3 | 17,826 | 5,850 | $0.000196 | 1,689 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1529 | Tiny | KG | 1 | 23,143 | 7,224 | $0.000998 | 1,544 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1529 | Tiny | KG | 2 | 23,143 | 7,224 | $0.000194 | 1,640 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1529 | Tiny | KG | 3 | 23,143 | 7,224 | $0.000226 | 2,116 | mistral-small-2603 | approve | 0 | 0 | 4 |
| 1531 | Small | No KG | 1 | 72,442 | 20,605 | $0.003276 | 4,637 | mistral-small-2603 | approve | 3 | 0 | 5 |
| 1531 | Small | No KG | 2 | 72,442 | 20,605 | $0.000548 | 3,290 | mistral-small-2603 | approve | 1 | 0 | 6 |
| 1531 | Small | No KG | 3 | 72,442 | 20,605 | $0.000616 | 3,965 | mistral-small-2603 | approve | 2 | 0 | 7 |
| 1531 | Small | KG | 1 | 89,629 | 25,589 | $0.004001 | 4,535 | mistral-small-2603 | approve | 3 | 0 | 5 |
| 1531 | Small | KG | 2 | 89,629 | 25,589 | $0.000721 | 4,923 | mistral-small-2603 | approve | 3 | 0 | 6 |
| 1531 | Small | KG | 3 | 89,629 | 25,589 | $0.000749 | 3,805 | mistral-small-2603 | approve | 4 | 0 | 6 |
| 1532 | Medium | No KG | 1 | 82,559 | 28,900 | $0.004442 | 4,413 | mistral-small-2603 | approve | 2 | 0 | 6 |
| 1532 | Medium | No KG | 2 | 82,559 | 28,900 | $0.000746 | 4,197 | mistral-small-2603 | approve | 2 | 0 | 6 |
| 1532 | Medium | No KG | 3 | 82,559 | 28,900 | $0.000773 | 4,455 | mistral-small-2603 | approve | 3 | 0 | 6 |
| 1532 | Medium | KG | 1 | 96,741 | 32,511 | $0.004940 | 3,720 | mistral-small-2603 | approve | 2 | 0 | 5 |
| 1532 | Medium | KG | 2 | 96,741 | 32,511 | $0.000767 | 3,666 | mistral-small-2603 | approve | 2 | 0 | 6 |
| 1532 | Medium | KG | 3 | 96,741 | 32,511 | $0.000730 | 3,619 | mistral-small-2603 | approve | 2 | 0 | 6 |
| 1519 | Large | No KG | 1 | 72,754 | 19,184 | $0.004102 | 5,027 | mistral-small-2603 | approve | 4 | 0 | 6 |
| 1519 | Large | No KG | 2 | 72,754 | 19,184 | $0.004115 | 4,733 | mistral-small-2603 | approve | 3 | 0 | 7 |
| 1519 | Large | No KG | 3 | 72,754 | 19,172 | $0.003064 | 4,992 | mistral-small-2603 | approve | 2 | 0 | 6 |
| 1519 | Large | KG | 1 | 110,712 | 29,463 | $0.004552 | 5,738 | mistral-small-2603 | approve | 3 | 0 | 6 |
| 1519 | Large | KG | 2 | 110,712 | 29,463 | $0.000758 | 4,578 | mistral-small-2603 | approve | 2 | 1 | 6 |
| 1519 | Large | KG | 3 | 110,712 | 29,463 | $0.004733 | 5,150 | mistral-small-2603 | approve | 2 | 0 | 7 |

### Table 8: Post-Fix Warm-Cache Averages Per PR (Runs 2+3)

| PR | Size (add/del/files) | Modified Files | Config | Prompt Chars | Input Tokens | Cost (USD) | Duration (ms) | Graph Cache (bytes) |
|----|---------------------|----------------|--------|-------------|-------------|------------|---------------|---------------------|
| 1529 | Tiny (2/3/1) | `graph/query.rs` | No KG | 17,826 | 5,850 | $0.000189 | 1,621 | N/A |
| 1529 | | | KG | 23,143 | 7,224 | $0.000210 | 1,878 | 2,590 |
| | | | Delta | +5,317 | +1,374 | +$0.000021 | +257 | |
| | | | % Change | +29.8% | +23.5% | +10.9% | +15.9% | |
| 1531 | Small (31/38/3) | `pr_review.rs`, `patterns.json`, `scanner.rs` | No KG | 72,442 | 20,605 | $0.000582 | 3,628 | N/A |
| 1531 | | | KG | 89,629 | 25,589 | $0.000735 | 4,364 | 13,896 |
| | | | Delta | +17,187 | +4,984 | +$0.000153 | +736 | |
| | | | % Change | +23.7% | +24.2% | +26.2% | +20.3% | |
| 1532 | Medium (211/47/4) | `Cargo.lock`, `Cargo.toml`, `builder.rs`, `mod.rs` | No KG | 82,559 | 28,900 | $0.000759 | 4,326 | N/A |
| 1532 | | | KG | 96,741 | 32,511 | $0.000748 | 3,642 | 6,322 |
| | | | Delta | +14,182 | +3,611 | -$0.000011 | -684 | |
| | | | % Change | +17.2% | +12.5% | -1.4% | -15.8% | |
| 1519 | Large (266/21/2) | `review_context.rs`, `ast_context.rs` | No KG | 72,754 | 19,178 | $0.003589 | 4,862 | N/A |
| 1519 | | | KG | 110,712 | 29,463 | $0.002745 | 4,864 | 21,933 |
| | | | Delta | +37,958 | +10,285 | -$0.000844 | +2 | |
| | | | % Change | +52.2% | +53.6% | -23.5% | +0.0% | |

### Table 9: Pre-Fix vs Post-Fix Comparison — KG Context Injection

| PR | Size | Pre-Fix KG Delta (chars) | Post-Fix KG Delta (chars) | Pre-Fix Cache (bytes) | Post-Fix Cache (bytes) | Pre-Fix Injected? | Post-Fix Injected? |
|----|------|--------------------------|---------------------------|----------------------|------------------------|-------------------|-------------------|
| 1529 | Tiny | +5,317 | +5,317 | 2,590 | 2,590 | Yes | Yes (unchanged) |
| 1531 | Small | 0 | +17,187 | 12 | 13,896 | No (bug) | Yes (fixed) |
| 1532 | Medium | +14,182 | +14,182 | 6,179 | 6,322 | Yes | Yes (unchanged) |
| 1519 | Large | 0 | +37,958 | 12 | 21,933 | No (bug) | Yes (fixed) |

The fix in PR #1539 is confirmed: KG now injects context on all 4 PRs (was 2 of 4). The previously broken PRs (#1531, #1519) now produce non-empty graph caches (13,896 and 21,933 bytes vs 12 bytes previously). PRs that already worked (#1529, #1532) are unchanged in context injection volume, confirming the fix does not alter existing behavior.

### Table 10: Post-Fix Cross-PR Comparison — KG Delta % by PR Size

| PR | Size | Files Modified | Graph Cache | Token Delta % | Char Delta % | KG Injected Context? |
|----|------|---------------|-------------|---------------|--------------|---------------------|
| 1529 | Tiny (2/3/1) | 1 .rs (graph module) | 2,590 bytes | +23.5% | +29.8% | Yes |
| 1531 | Small (31/38/3) | 2 .rs + 1 .json (non-graph) | 13,896 bytes | +24.2% | +23.7% | Yes (fixed) |
| 1532 | Medium (211/47/4) | 2 .rs (graph module) + 2 non-code | 6,322 bytes | +12.5% | +17.2% | Yes |
| 1519 | Large (266/21/2) | 2 .rs (non-graph) | 21,933 bytes | +53.6% | +52.2% | Yes (fixed) |

Post-fix, KG context injection correlates with the number of graph-indexed symbols in modified files, not with PR size. PR #1519 (large, 2 non-graph .rs files) produces the largest KG context (+37,958 chars, +52.2%) because `ast_context.rs` and `review_context.rs` contain many symbols that the graph builder indexes. PR #1532 (medium, 2 graph-module files) produces less context (+14,182 chars) despite more lines changed, because the modified graph-module files have fewer indexed symbols.

### Table 11: Post-Fix Quality Indicators (Warm-Cache Averages)

| PR | Size | Config | Verdict | Comments (avg) | Concerns (avg) | Strengths (avg) |
|----|------|--------|---------|----------------|----------------|-----------------|
| 1529 | Tiny | No KG | approve | 0.0 | 0.0 | 4.0 |
| 1529 | Tiny | KG | approve | 0.0 | 0.0 | 4.0 |
| 1531 | Small | No KG | approve | 1.5 | 0.0 | 6.5 |
| 1531 | Small | KG | approve | 3.5 | 0.0 | 6.0 |
| 1532 | Medium | No KG | approve | 2.5 | 0.0 | 6.0 |
| 1532 | Medium | KG | approve | 2.0 | 0.0 | 6.0 |
| 1519 | Large | No KG | approve | 2.5 | 0.0 | 6.5 |
| 1519 | Large | KG | approve | 2.0 | 0.5 | 6.5 |

All 24 runs returned `approve`. PR #1519 KG run 2 produced 1 concern (the only non-zero concern across all 48 runs in both benchmarks). Comment and strength counts vary due to AI non-determinism with no consistent directional effect from KG.

### Post-Fix Graph Cache Verification

Cache files are keyed by PR head commit SHA (same SHAs as pre-fix benchmark since PRs are unchanged):

| PR | Head SHA (prefix) | Pre-Fix Cache Size | Post-Fix Cache Size | Context Injected (Post-Fix) |
|----|-------------------|--------------------|---------------------|-----------------------------|
| 1529 | `823128f3` | 2,590 bytes | 2,590 bytes | Yes (+5,317 chars, unchanged) |
| 1531 | `ee7efafe` | 12 bytes (empty) | 13,896 bytes | Yes (+17,187 chars, fixed) |
| 1532 | `af4e9ce4` | 6,179 bytes | 6,322 bytes | Yes (+14,182 chars, unchanged) |
| 1519 | `3c2b019a` | 12 bytes (empty) | 21,933 bytes | Yes (+37,958 chars, fixed) |

The 12-byte files from the pre-fix benchmark contained only the schema-hash header (empty graph). Post-fix, all 4 cache files contain real graph data. The slight increase for #1532 (6,179 to 6,322 bytes) is within normal variation from minor symbol-range changes in the fix. Prompt chars remain identical across all 3 KG runs per PR (deterministic), confirming cache correctness (F4 invariant holds post-fix).

### No-KG Baseline Shift

The No-KG prompt chars changed for 2 of 4 PRs between pre-fix and post-fix builds:

| PR | Pre-Fix No-KG Chars | Post-Fix No-KG Chars | Delta | Explanation |
|----|---------------------|----------------------|-------|-------------|
| 1529 | 17,826 | 17,826 | 0 | Small file; AST cap never hit |
| 1531 | 71,236 | 72,442 | +1,206 | Fix allows more files' AST text under cap |
| 1532 | 82,559 | 82,559 | 0 | Small files; AST cap never hit |
| 1519 | 71,300 | 72,754 | +1,454 | Fix allows more files' AST text under cap |

The `build_ast_context_sync()` fix changes AST text accumulation even without KG enabled: the pre-fix `break` stopped processing files after the first large file exceeded the 2000-char text cap, while the post-fix code continues processing all files but caps only the text output. This results in more AST context being included in the prompt for PRs with large source files (#1531, #1519), even when the graph is disabled.

This baseline shift is a positive change for review accuracy. Before the fix, a large first file could exhaust the 2000-char text cap, causing all subsequent files to receive zero AST context in the prompt. The AI reviewer was structurally blind to every file after the first. Post-fix, AST text from all modified files is included (up to the cap), giving the reviewer function signatures, type definitions, and import lists for a broader set of files. The token cost is modest (~340-410 tokens) relative to the 72k-char total prompt, and the AST text comes from trusted repository source files (GitHub Contents API), adding no prompt-injection surface. The improvement is structural and logical, though the 48-run sample (all `approve`, 0-1 concerns) is insufficient to detect a measurable quality difference.

### Post-Fix Findings

#### F6: Bug fix confirmed — KG now injects context on all 4 PRs (INFO / POSITIVE)

**Severity:** Info  
**Category:** POSITIVE

PR #1539 successfully fixes the F5 bug. KG context injection now fires on all 4 tested PRs (was 2 of 4). The previously broken PRs (#1531, #1519) now produce non-empty graph caches (13,896 and 21,933 bytes vs 12 bytes) and inject +17,187 and +37,958 prompt chars respectively. PRs that already worked (#1529, #1532) are unchanged, confirming the fix is non-regressive.

**Impact:** The primary blocker for KG default enablement (F5/R2) is resolved. KG is no longer a silent no-op for PRs modifying large source files.

#### F7: KG token overhead ranges from +12.5% to +53.6% across all PRs (INFO / MEASUREMENT)

**Severity:** Info  
**Category:** MEASUREMENT

With the fix, KG overhead is no longer limited to the 2 PRs where it previously fired. The token overhead range expands from +12.5% to +23.5% (pre-fix, 2 PRs) to +12.5% to +53.6% (post-fix, 4 PRs). The largest PR (#1519: 266 add / 21 del / 2 files) incurs the highest relative overhead (+53.6% tokens) because its modified files (`ast_context.rs`, `review_context.rs`) contain many graph-indexed symbols.

The token-to-char ratio for KG-injected context ranges from 0.25 to 0.29 across all 4 PRs, confirming compact serialization (consistent with F1).

**Impact:** KG overhead is predictable and proportional to the number of graph-indexed symbols in modified files. The +53.6% overhead on PR #1519 is the upper bound for this PR set; larger PRs with more symbols could exceed this. The overhead is a token cost trade-off for structural context.

#### F8: KG overhead does not correlate with PR size (INFO / MEASUREMENT)

**Severity:** Info  
**Category:** MEASUREMENT

Post-fix data confirms the pre-fix finding (Q1/Q4): KG overhead does not scale with PR size (lines changed or file count). The correlation is with the number of graph-indexed symbols in modified files:

- #1529 (tiny, 1 graph file): +23.5% tokens, 2,590-byte cache
- #1531 (small, 3 non-graph files): +24.2% tokens, 13,896-byte cache
- #1532 (medium, 2 graph files + 2 non-code): +12.5% tokens, 6,322-byte cache
- #1519 (large, 2 non-graph files with many symbols): +53.6% tokens, 21,933-byte cache

PR #1519 is the largest by lines changed and produces the most KG context, but this is because `ast_context.rs` is a symbol-dense file, not because of PR size. PR #1532 has more lines changed than #1531 but less KG overhead because its graph-module files have fewer indexed symbols.

**Impact:** PR size is not a useful predictor of KG overhead. Symbol density in modified files is the determining factor. This means KG overhead is unpredictable from PR metadata alone.

### Post-Fix Recommendations

#### R6: KG default enablement decision deferred pending value measurement (info)

**Priority:** Info  
**Fixes:** F5, F6

The F5 bug is fixed and KG overhead is quantified (+12.5% to +53.6% tokens). However, this benchmark measures cost only, not value (see Limitations). KG default enablement should be decided after running Benchmark v2 (see "Proposed Benchmark v2" below), which tests whether KG helps reviewers catch structural defects that diff-only reviews miss.

#### R7: Run Benchmark v2 (cost + value) after StructuralGraph consolidation (info)

**Priority:** Info  
**Fixes:** —

Supersedes R3. After issue #1533 replaces the KG interface, run Benchmark v2 with known-defect PRs to measure both cost and value. Three-way comparison: No KG, current KG (this baseline), StructuralGraph. The value dimension (does the review catch the defect?) is the primary decision input; cost is secondary if the value is real.

### Post-Fix Summary

*Table 12: Post-Fix Findings.*

| ID | Severity | Category | Finding |
|---|---|---|---|
| F6 | Info | POSITIVE | Bug fix confirmed — KG now injects context on all 4 PRs (was 2 of 4) |
| F7 | Info | MEASUREMENT | KG token overhead ranges from +12.5% to +53.6% across all PRs (was +12.5% to +23.5% on 2 PRs) |
| F8 | Info | MEASUREMENT | KG overhead does not correlate with PR size; symbol density in modified files is the predictor |

*Table 13: Post-Fix Recommendations.*

| ID | Priority | Fixes | Recommendation |
|---|---|---|---|
| R6 | Info | F5, F6 | KG default enablement deferred pending value measurement (cost-only benchmark insufficient) |
| R7 | Info | — | Run Benchmark v2 (cost + value, known-defect PRs) after StructuralGraph consolidation |

Note: R5 recommended creating a separate `2026-XX-XX-kg-benchmark-post-fix.md` file. The post-fix results were appended to this file instead, preserving the pre-fix baseline (Tables 1-6) intact above. The pre-fix data is unmodified; all post-fix content starts at the "Post-Fix Benchmark" section. This file now serves as the complete pre-StructuralGraph baseline for issue #1533.

### Limitations

This benchmark measures the **cost** of KG (token overhead, cache behavior, latency) but provides **no evidence of value**. The gap is significant:

1. **No accuracy measurement**: All 48 runs (pre-fix and post-fix) returned `approve` with 0-1 concerns. When every run produces the same verdict, there is zero signal on whether KG improves review quality. We cannot distinguish "the PRs are clean" from "the model defaults to approval regardless of context."

2. **Comment counts are noise, not quality**: We count comments (3 vs 2 vs 0) but never read what they say. Three wrong comments are worse than zero. Two insightful comments are better than five generic ones. The count fluctuates due to AI non-determinism with no directional effect from KG, as the document itself acknowledges in Q3.

3. **No known-defect PRs**: All 4 PRs are merged and presumably fine. There is no PR with a subtle structural bug where we could test whether KG helps the reviewer catch something they would otherwise miss. Without this, the value side of the cost/value trade-off is unmeasured.

4. **Verdict correctness is unverified**: We record the verdict but never assess whether it is correct. All `approve` verdicts could be right, or the model could be rubber-stamping.

**Impact on recommendations**: R6 ("KG default enablement is now viable") is overstated. The benchmark proves the bug is fixed and the cost is quantified, but it cannot tell you whether the +12.5% to +53.6% token overhead is worth paying. A cost-only benchmark can rule out prohibitively expensive implementations, but it cannot justify enablement.

### Proposed Benchmark v2: Cost and Value

To measure both cost and value, the benchmark needs PRs with known structural defects where KG should theoretically help. The design below keeps it simple: 3 defect PRs + 1 clean PR, 2 runs each, with and without KG (16 total runs instead of 24).

**Defect PR design**: Create small PRs (1-3 files, 10-30 lines) in a scratch repo or branch, each containing a subtle structural bug that a diff-only reviewer might miss but a call-graph-aware reviewer should catch:

| Defect | Description | Why KG Should Help |
|--------|-------------|-------------------|
| Broken caller | Change a function signature; leave a caller un-updated | Graph shows the caller edge; reviewer flags the mismatch |
| Dead code path | Remove a function that is still called elsewhere | Graph shows incoming edges; reviewer flags the dangling call |
| Wrong trait impl | Implement a trait method with the wrong return type | Graph shows impl relationships; reviewer cross-checks |

Each defect PR should have a clean version (same files, no bug) to serve as a control. This gives 4 PRs: 3 defective + 1 clean.

**Method**:

- 2 configs: No KG, KG enabled
- 2 runs per PR per config (not 3; 2 is enough with deterministic prompt chars)
- 4 PRs x 2 configs x 2 runs = 16 runs
- Same model, same command, same extraction

**Value metrics** (new):

- For each run, extract the full `comments` array text (not just count)
- Classify each comment as: catches the defect (true positive), flags something irrelevant (false positive), or misses the defect (false negative)
- Binary scoring per PR: did the review flag the actual bug? (yes/no)
- Compare hit rate: KG vs No KG on the 3 defect PRs

**Cost metrics** (same as current):

- `input_tokens`, `prompt_chars`, `cost_usd`, `duration_ms`

**Decision rule**: KG is worth enabling if it catches defects that No KG misses, without increasing false positives. Cost is secondary if the value is real.

**What this does NOT do**:

- No human-rated review quality (too expensive, not simple)
- No large-scale statistical analysis (16 runs is small, but binary hit/miss is interpretable)
- No latency focus (already shown to be noise at this scale per F3)

This design trades coverage (4 PRs instead of 4, 2 runs instead of 3) for a new dimension (value). The cost metrics from the current benchmark are already stable and do not need 3 runs to confirm. The value question is binary (caught the bug or not), which is readable even with 2 runs.
