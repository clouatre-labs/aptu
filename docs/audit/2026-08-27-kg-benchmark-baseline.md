# Audit: KG Benchmark Baseline (No KG vs Current KG) — August 2026

Date: 2026-08-27
Toolchain: aptu 0.10.16 release build (main @ 88c20e4), aptu-coder-core 0.32.2 (pre-consolidation)
Data: PR clouatre-labs/aptu#1532, 3 runs x 2 conditions (graph disabled, graph enabled), same
OpenRouter `mistralai/mistral-small-2603` model on both sides.
Method: `aptu pr review 1532 --repo clouatre-labs/aptu -o json` — real AI calls, nothing posted.
Scope: `~/.config/aptu/config.toml` (`[graph]` section), `~/.local/share/aptu/graph/` (disk cache),
`.ai_stats` JSON fields (`prompt_chars`, `input_tokens`, `cost_usd`, `duration_ms`).

---

## Purpose

Establish two performance baselines for aptu PR reviews before the StructuralGraph
consolidation (planned in #1534):

1. **No KG** — graph disabled (default; no config file)
2. **Current KG** — aptu-coder-core 0.32.2 graph enabled with pre-consolidation
   `graph::builder` + `graph::query`

After PR 2 ships the consolidation, re-run this benchmark with a third config ("future KG" using
`StructuralGraph`). The three-way comparison will show whether the migration improved, regressed,
or maintained KG quality and cost.

---

## Method

3 runs per config (6 total). Command: `aptu pr review 1532 --repo clouatre-labs/aptu -o json`.
Extracted from JSON: `.ai_stats.prompt_chars`, `.ai_stats.input_tokens`, `.ai_stats.cost_usd`,
`.ai_stats.duration_ms`.

Graph cache cleared between configs (`rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/`),
not between runs of the same config (runs 2/3 should hit cache).

The `-o json` flag produces the review locally without posting to GitHub. PR #1532 is already
merged; this is a read-only benchmark.

---

## Reproduction

### Prerequisites

- aptu installed at `~/.cargo/bin/aptu` (release build, main branch at 88c20e4)
- GitHub auth configured (OAuth device flow via `aptu auth login`)
- No config file at `~/.config/aptu/config.toml` (KG disabled by default)

### No-KG config

No config file needed. Clear graph cache and run:

```bash
rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/
aptu pr review 1532 --repo clouatre-labs/aptu -o json 2>/dev/null \
  | jq '{prompt_chars: .ai_stats.prompt_chars, input_tokens: .ai_stats.input_tokens,
         cost_usd: .ai_stats.cost_usd, duration_ms: .ai_stats.duration_ms}'
```

Repeat 3 times. Do NOT clear cache between runs.

### KG-enabled config

Create `~/.config/aptu/config.toml`:

```toml
[graph]
enabled = true
max_depth = 4
max_nodes = 50000
cache_ttl_hours = 24
```

Clear graph cache and run:

```bash
rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/
aptu pr review 1532 --repo clouatre-labs/aptu -o json 2>/dev/null \
  | jq '{prompt_chars: .ai_stats.prompt_chars, input_tokens: .ai_stats.input_tokens,
         cost_usd: .ai_stats.cost_usd, duration_ms: .ai_stats.duration_ms}'
```

Repeat 3 times. Do NOT clear cache between runs.

### Cleanup

```bash
rm -f ~/.config/aptu/config.toml
```

---

## Results

*Table 1: All runs, 3 per config.*

| Run | Config | Prompt Chars | Input Tokens | Cost (USD) | Duration (ms) | Graph Cache | AI Provider Cache |
|-----|--------|-------------|-------------|------------|---------------|-------------|-------------------|
| 1 | No KG | 82,559 | 28,900 | $0.004696 | 5,642 | N/A | cold |
| 2 | No KG | 82,559 | 28,900 | $0.000701 | 4,417 | N/A | warm |
| 3 | No KG | 82,559 | 28,900 | $0.000721 | 4,288 | N/A | warm |
| 4 | KG enabled | 96,741 | 32,511 | $0.001361 | 4,570 | cold | cold |
| 5 | KG enabled | 96,741 | 32,511 | $0.000746 | 4,812 | warm | warm |
| 6 | KG enabled | 96,741 | 32,511 | $0.000732 | 3,750 | warm | warm |

*Table 2: Averages per config.*

| Metric | No KG (avg) | KG Enabled (avg) |
|--------|------------|-----------------|
| Prompt chars | 82,559 | 96,741 |
| Input tokens | 28,900 | 32,511 |
| Cost (USD) | $0.002039 | $0.000946 |
| Duration (ms) | 4,782 | 4,377 |

*Table 3: Delta (KG vs No KG).*

| Metric | Delta | % Change |
|--------|-------|----------|
| Prompt chars | +14,182 | +17.2% |
| Input tokens | +3,611 | +12.5% |
| Cost (USD) | -$0.001093 | -53.6% |
| Duration (ms) | -405 | -8.5% |

*Table 4: Warm-cache comparison (runs 2/3 vs 5/6 only).*

The first run of each config pays cold AI provider cache costs, which dominate variance. For a
fair comparison, use warm-cache runs only:

| Metric | No KG (warm avg) | KG (warm avg) | Delta | % Change |
|--------|-----------------|--------------|-------|----------|
| Prompt chars | 82,559 | 96,741 | +14,182 | +17.2% |
| Input tokens | 28,900 | 32,511 | +3,611 | +12.5% |
| Cost (USD) | $0.000711 | $0.000739 | +$0.000028 | +3.9% |
| Duration (ms) | 4,353 | 4,281 | -72 | -1.7% |

### Plain reading of the results

- KG adds 14,182 prompt chars (+17.2%) of structural graph context (blast-radius traversal, call
  graph) to the review prompt, consistent with the graph populating for this PR (it modifies
  graph module files, so `find_modified_nodes` returns non-empty results).
- Token increase is proportionally smaller (+12.5%) than char increase, suggesting the graph
  context is token-efficient (compact serialization).
- Cost impact is minimal on warm cache (+3.9%), masked entirely by AI provider prompt caching
  which dominates cost variance.
- Latency impact is negligible (-1.7% warm). Graph cache build/load is not the bottleneck; AI
  API round-trip dominates.
- Graph cache works correctly: identical prompt chars across cold/warm runs confirms deterministic
  context extraction. Cache file persisted to disk (1 file, 6,404 bytes) and was reused.

---

## Findings

### F1: KG adds 17.2% prompt chars with proportional token cost (INFO / POSITIVE)

**Severity:** Info
**Category:** POSITIVE

KG context injection adds 14,182 prompt chars and 3,611 input tokens. The token-to-char ratio
for graph context (3,611 tokens / 14,182 chars = 0.25) is lower than the overall prompt ratio
(28,900 / 82,559 = 0.35), indicating compact serialization.

**Impact:** The feature delivers structural context at a predictable, modest token cost. This is
the baseline to compare against `StructuralGraph` post-consolidation.

### F2: cost_usd deltas are not a reliable signal (INFO / MEASUREMENT)

**Severity:** Info
**Category:** MEASUREMENT

The -53.6% average cost delta is an artifact of the no-KG run 1 cold-cache outlier ($0.0047 vs
$0.0014 for KG run 1). The KG run 1 was cheaper despite more tokens, possibly due to model-tier
routing or provider-side prompt caching variance. The warm-cache comparison (runs 2/3 vs 5/6) is
the reliable cost metric: +3.9% for +12.5% tokens.

**Impact:** `cost_usd` should not be used as the primary metric for KG impact assessment.
`input_tokens` is the trustworthy metric, consistent with F3 in the
2026-08-26 graph-context-prompt-injection audit.

### F3: graph cache hit does not reliably reduce latency (INFO / MEASUREMENT)

**Severity:** Info
**Category:** MEASUREMENT

KG run 5 latency (4,812 ms) was higher than run 4 (4,570 ms) despite warm graph cache. Run 6
(3,750 ms) was the fastest overall. Graph build is fast enough that its contribution is within
network/AI latency noise.

**Impact:** Latency is not a useful metric for graph cache effectiveness at this scale. Prompt
chars and input tokens are deterministic and should be the primary comparison axes for the
post-consolidation benchmark.

### F4: graph cache produces deterministic context across cold/warm runs (INFO / POSITIVE)

**Severity:** Info
**Category:** POSITIVE

Prompt chars are identical across all 3 KG runs (96,741), confirming the cache returns the same
context as a fresh build. Cache file persisted to disk and was loaded on runs 2/3.

**Impact:** Graph cache correctness is confirmed. Post-consolidation, the same invariant must
hold: `StructuralGraph` cache must produce identical prompt chars across cold/warm runs.

---

## Recommendations

### R1: Re-run benchmark after StructuralGraph consolidation (info)

**Priority:** Info
**Fixes:** —

Re-run with the same PR (#1532), same method, 3 runs. Add a third config ("future KG" using
`StructuralGraph`). Compare three-way: prompt chars, input tokens, cost, latency. Key question:
does `StructuralGraph::bfs_blast_radius()` (outgoing-only) produce the same or different context
volume as the current bidirectional `graph::query::blast_radius()`? If outgoing-only produces
less context, expect lower prompt chars and tokens.

### R2: Use input_tokens as primary metric, not cost_usd (info)

**Priority:** Info
**Fixes:** F2

`cost_usd` is dominated by AI provider prompt caching variance. Use `input_tokens` as the
primary comparison axis for all future KG benchmarks. Report `cost_usd` only with warm-cache
averages and an explicit caveat.

### R3: Preserve this file as the baseline (info)

**Priority:** Info
**Fixes:** —

Create `2026-XX-XX-kg-benchmark-post-consolidation.md` for the follow-up. Do not modify this
file after merge; it is the pre-consolidation baseline.

---

## Summary

*Table 5: Findings.*

| ID | Severity | Category | Finding |
|---|---|---|---|
| F1 | Info | POSITIVE | KG adds 17.2% prompt chars with proportional, token-efficient cost |
| F2 | Info | MEASUREMENT | cost_usd deltas unreliable; use input_tokens as primary metric |
| F3 | Info | MEASUREMENT | Graph cache hit does not reliably reduce latency at this scale |
| F4 | Info | POSITIVE | Graph cache produces deterministic context across cold/warm runs |

*Table 6: Recommendations.*

| ID | Priority | Fixes | Recommendation |
|---|---|---|---|
| R1 | Info | — | Re-run benchmark after StructuralGraph consolidation (three-way comparison) |
| R2 | Info | F2 | Use input_tokens as primary metric, not cost_usd |
| R3 | Info | — | Preserve this file as the pre-consolidation baseline |
