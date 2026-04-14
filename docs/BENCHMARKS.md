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
