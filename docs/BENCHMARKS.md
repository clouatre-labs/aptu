<!-- SPDX-FileCopyrightText: 2026 Aptu Contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Benchmarks

This document records the prompt size reduction and quality verification from the system prompt compression initiative (PRs #1103-#1105).

## Methodology

**Prompt Measurement:** `bench/measure.sh` measures system prompt character counts per operation (triage, pr_review, create, release, pr_label). Measurements are taken as aggregate character counts across persona, tooling, and guidelines sections.

**Quality Rubric:** `bench/rubric.md` defines a binary C1-C5 scoring rubric with these criteria:
- C1: Precision (no false positives)
- C2: Recall (catches real issues)
- C3: Scope (matches issue scope)
- C4: Actionability (fixes are clear)
- C5: Tone (constructive and encouraging)

Threshold: **>= 4/5 on all non-exempt fixtures** to pass the quality gate.

**Evaluator:** `inception/mercury-2` via OpenRouter. Evaluations are post-compression only; baseline comparisons are informational.

## Prompt Size

| Operation | Before (chars) | After (chars) | Reduction |
|-----------|----------------|---------------|-----------|
| triage | 4,757 | 3,337 | −29.9% |
| pr_review | 4,704 | 2,938 | −37.5% |
| create | 3,571 | 2,534 | −29.0% |
| release | 3,945 | 2,785 | −29.4% |
| pr_label | 2,467 | 1,707 | −30.8% |
| **Total** | **19,444** | **13,301** | **−31.6%** |

## Quality Scores

| Fixture | Type | Score | Result |
|---------|------|-------|--------|
| clouatre-labs/aptu#1091 | pr_review | 5/5 | PASS |
| clouatre-labs/aptu#1098 | pr_review | 5/5 | PASS |
| clouatre-labs/aptu#1101 | pr_review | 5/5 | PASS |
| clouatre-labs/aptu#850 | triage | 4/5 | PASS |
| clouatre-labs/aptu#1094 | triage | 5/5 | PASS |
| clouatre-labs/aptu#737 | triage | 3/5 | exempted (closed/wontfix by design) |

Evaluator: `inception/mercury-2` via OpenRouter. Threshold: >= 4/5 on all non-exempt fixtures.

## References

- [#1103](https://github.com/clouatre-labs/aptu/pull/1103) compress prompt files (−34% chars)
- [#1104](https://github.com/clouatre-labs/aptu/pull/1104) record post-#1103 prompt size measurements
- [#1105](https://github.com/clouatre-labs/aptu/pull/1105) record quality smoke-test scores

## Comparative Benchmark

Head-to-head comparison of `aptu+mercury-2` (structured, schema-enforced triage/review) vs a raw `claude-opus-4.6` call with a two-sentence generic prompt (no schema, no rubric, no AST context). Issue [#1122](https://github.com/clouatre-labs/aptu/issues/1122).

### Setup

| Arm | Model | Provider | Prompt |
|-----|-------|----------|--------|
| aptu+mercury-2 | `inception/mercury-2` | openrouter | Full aptu structured prompt (schema, rubric, AST context) |
| raw_opus46 | `anthropic/claude-opus-4.6` | openrouter | Two-sentence generic prompt; no schema, no rubric, no AST context |

### Quality Scores

Rubric: [bench/rubric.md](https://github.com/clouatre-labs/aptu/blob/main/bench/rubric.md). C5=0 for raw_baseline by design (no schema enforcement).

| Fixture | Type | aptu C1-C5 | aptu Score | raw C1-C5 | raw Score |
|---------|------|------------|------------|-----------|-----------|
| [#737](https://github.com/clouatre-labs/aptu/issues/737) | triage | 1,1,0,0,1 | 3/5 (exempted) | 1,1,0,0,0 | 2/5 |
| [#850](https://github.com/clouatre-labs/aptu/issues/850) | triage | 1,1,0,1,1 | 4/5 | 1,1,0,0,0 | 2/5 |
| [#1094](https://github.com/clouatre-labs/aptu/issues/1094) | triage | 1,1,1,1,1 | 5/5 | 1,1,0,0,0 | 2/5 |
| [#1101](https://github.com/clouatre-labs/aptu/pulls/1101) | pr_review | 1,1,1,1,1 | 5/5 | 0,1,0,1,0 | 2/5 |
| [#1098](https://github.com/clouatre-labs/aptu/pulls/1098) | pr_review | 1,1,1,1,1 | 5/5 | 0,1,0,1,0 | 2/5 |
| [#1091](https://github.com/clouatre-labs/aptu/pulls/1091) | pr_review | 1,1,1,1,1 | 5/5 | 1,1,0,1,0 | 3/5 |

Mean (non-exempt triage + all pr_review): aptu **4.8/5**, raw_opus46 **2.2/5**.

### Efficiency

| Arm | Cost/call (mean) | Latency p50 |
|-----|------------------|-------------|
| aptu+mercury-2 | $0.001135 | 1,934 ms |
| raw_opus46 | $0.019254 | 16,032 ms |

aptu+mercury-2 is **17x cheaper** and **8x faster** than a raw opus-4.6 call, while scoring more than twice as high on the structured rubric.

### Methodology

- Evaluator: human rubric evaluation against [bench/rubric.md](https://github.com/clouatre-labs/aptu/blob/main/bench/rubric.md)
- n=1 per fixture (6 total samples per arm); latency p50 computed across 6 fixtures, not repeated runs
- aptu arm cost from `ai_stats.cost_usd` in `--output json --dry-run` response
- raw_opus46 cost from `usage.cost_details.upstream_inference_cost` (OpenRouter BYOK mode returns top-level cost=0)
- Raw data: [bench/results/efficiency.json](https://github.com/clouatre-labs/aptu/blob/main/bench/results/efficiency.json), [bench/results/scores.json](https://github.com/clouatre-labs/aptu/blob/main/bench/results/scores.json)
