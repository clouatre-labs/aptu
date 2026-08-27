# Audit: KG Benchmark Baseline (No KG vs Current KG)

Date: 2026-08-27
Binary: aptu 0.10.16 (main @ 88c20e4, release build)
Target PR: clouatre-labs/aptu#1532 (graph module changes, triggers KG context injection)
KG version: aptu-coder-core 0.32.2 (pre-consolidation)
Related: #1510, #1528, #1532, #1534

## Purpose

Establish two performance baselines for aptu PR reviews before the StructuralGraph consolidation (planned in #1534):

1. **No KG** - graph disabled (default; no config file)
2. **Current KG** - aptu-coder-core 0.32.2 graph enabled with pre-consolidation `graph::builder` + `graph::query`

After PR 2 ships the consolidation, re-run this benchmark with a third config ("future KG" using `StructuralGraph`). The three-way comparison will show whether the migration improved, regressed, or maintained KG quality and cost.

## Method

- 3 runs per config (6 total)
- Command: `aptu pr review 1532 --repo clouatre-labs/aptu -o json`
- Extracted metrics from JSON: `.ai_stats.prompt_chars`, `.ai_stats.input_tokens`, `.ai_stats.cost_usd`, `.ai_stats.duration_ms`
- Graph cache cleared between configs (`rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/`), NOT between runs of the same config (runs 2/3 should hit cache)
- The `-o json` flag produces the review locally without posting to GitHub

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
  | jq '{prompt_chars: .ai_stats.prompt_chars, input_tokens: .ai_stats.input_tokens, cost_usd: .ai_stats.cost_usd, duration_ms: .ai_stats.duration_ms}'
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
  | jq '{prompt_chars: .ai_stats.prompt_chars, input_tokens: .ai_stats.input_tokens, cost_usd: .ai_stats.cost_usd, duration_ms: .ai_stats.duration_ms}'
```

Repeat 3 times. Do NOT clear cache between runs.

### Cleanup

```bash
rm -f ~/.config/aptu/config.toml
```

## Results

### All Runs

| Run | Config | Prompt Chars | Input Tokens | Cost (USD) | Duration (ms) | Graph Cache | AI Provider Cache |
|-----|--------|-------------|-------------|------------|---------------|-------------|-------------------|
| 1 | No KG | 82,559 | 28,900 | $0.004696 | 5,642 | N/A | cold |
| 2 | No KG | 82,559 | 28,900 | $0.000701 | 4,417 | N/A | warm |
| 3 | No KG | 82,559 | 28,900 | $0.000721 | 4,288 | N/A | warm |
| 4 | KG enabled | 96,741 | 32,511 | $0.001361 | 4,570 | cold | cold |
| 5 | KG enabled | 96,741 | 32,511 | $0.000746 | 4,812 | warm | warm |
| 6 | KG enabled | 96,741 | 32,511 | $0.000732 | 3,750 | warm | warm |

### Averages Per Config

| Metric | No KG (avg) | KG Enabled (avg) |
|--------|------------|-----------------|
| Prompt chars | 82,559 | 96,741 |
| Input tokens | 28,900 | 32,511 |
| Cost (USD) | $0.002039 | $0.000946 |
| Duration (ms) | 4,782 | 4,377 |

### Delta (KG vs No KG)

| Metric | Delta | % Change |
|--------|-------|----------|
| Prompt chars | +14,182 | +17.2% |
| Input tokens | +3,611 | +12.5% |
| Cost (USD) | -$0.001093 | -53.6% |
| Duration (ms) | -405 | -8.5% |

### Warm-Cache Comparison (Runs 2/3 vs 5/6)

The first run of each config pays cold AI provider cache costs, which dominate variance. For a fair comparison, use warm-cache runs only:

| Metric | No KG (warm avg) | KG (warm avg) | Delta | % Change |
|--------|-----------------|--------------|-------|----------|
| Prompt chars | 82,559 | 96,741 | +14,182 | +17.2% |
| Input tokens | 28,900 | 32,511 | +3,611 | +12.5% |
| Cost (USD) | $0.000711 | $0.000739 | +$0.000028 | +3.9% |
| Duration (ms) | 4,353 | 4,281 | -72 | -1.7% |

### Cache Analysis

**Graph cache (KG runs only):**
- Run 4 (cold): graph built from scratch, cached to disk (1 file, 6,404 bytes at `~/.local/share/aptu/graph/clouatre-labs/aptu/`)
- Runs 5/6 (warm): graph loaded from cache. Prompt chars identical across all 3 runs, confirming deterministic context extraction
- Graph cache hit does not reliably reduce wall-clock latency at this scale. The graph build is fast enough that its contribution is within network/AI latency noise

**AI provider prompt cache (both configs):**
- First run of each config pays full input token cost (cold)
- Runs 2/3 benefit from warm AI provider cache, reducing cost ~6x (no-KG: $0.0047 to $0.0007; KG: $0.0014 to $0.0007)

## Key Findings

1. **KG adds 14,182 prompt chars (+17.2%)** of structural graph context (blast-radius traversal, call graph) to the review prompt
2. **Token increase is proportionally smaller (+12.5%)** than char increase, suggesting the graph context is token-efficient (compact serialization)
3. **Cost impact is minimal on warm cache** (+3.9%), masked entirely by AI provider prompt caching which dominates cost variance
4. **Latency impact is negligible** (-1.7% warm, -8.5% average). Graph cache build/load is not the bottleneck; AI API round-trip dominates
5. **Graph cache works correctly**: identical prompt chars across cold/warm runs confirms deterministic context extraction. Cache file persisted to disk and was reused

## Anomalies

- The -53.6% average cost delta is an artifact of the no-KG run 1 cold-cache outlier ($0.0047 vs $0.0014 for KG run 1). The KG run 1 was cheaper despite more tokens, possibly due to model-tier routing selecting a different model for the larger prompt. The warm-cache comparison (runs 2/3 vs 5/6) is the reliable cost metric.
- KG run 5 latency (4,812 ms) was higher than run 4 (4,570 ms) despite warm graph cache. This is within network jitter; run 6 (3,750 ms) was the fastest overall.

## Future Benchmark Plan

After the StructuralGraph consolidation ships:

1. Re-run this benchmark with the same PR (#1532), same method, 3 runs
2. Add a third row to all tables: "Future KG (StructuralGraph)"
3. Compare three-way: prompt chars, tokens, cost, latency
4. Key question: does `StructuralGraph::bfs_blast_radius()` (outgoing-only) produce the same or different context volume as the current bidirectional `graph::query::blast_radius()`? If outgoing-only produces less context, expect lower prompt chars and tokens
5. Preserve this file as the baseline; create `2026-XX-XX-kg-benchmark-post-consolidation.md` for the follow-up
