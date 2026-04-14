# Benchmark Protocol

**Overview**
This document defines the protocol for validating the prompt compression refactor introduced in PR #1096. It measures the size of system prompts (persona, tooling context, and guidelines) before the refactor and provides a quality smoke‑test to ensure that the compressed prompts still produce correct behavior.

## Measurement (no API key required)
Run the measurement script:

```bash
bash bench/measure.sh
```
The script prints a markdown table with byte counts for each operation and writes `bench/results/sizes.json`.

## Quality Smoke‑Test

### Fixtures
- **Triage**: aptu #737, #800, #850
- **PR Review**: three recently merged aptu PRs (see `bench/protocol.md` section 2 for selection criteria)

### Procedure
1. Run the measurement script on the current (pre‑#1096) codebase and record the sizes.
2. Apply the changes from PR #1096 (prompt compression).
3. Run the measurement script again and record the new sizes.
4. Blindly score the before/after outputs using the rubric in `bench/rubric.md`.

### Scoring
Score each fixture using the evaluation rubric (`bench/rubric.md`). A pass requires a score of **4/5** on all fixtures in both groups.

## Results
Reference the generated JSON files:
- `bench/results/sizes.json`
- `bench/results/scores.json`

## Before Baseline Preservation

To capture a valid `before` baseline for future prompt-changing PRs:

1. Before merging the PR, run `bash bench/measure.sh` on the base branch and commit the updated `bench/results/sizes.json`.
2. For each triage fixture, run `aptu issue triage <ref> --output json --dry-run > bench/results/before-triage-<ref>.json`.
3. For each PR review fixture, run `aptu pr review <ref> --output json --dry-run > bench/results/before-pr-review-<ref>.json`.
4. Commit the captured outputs. After the PR merges, repeat steps 2-3 for `after` outputs, then score both sets against `bench/rubric.md`.

If the before state is missed (e.g. PR already merged), record only `after` scores and annotate the `before` arrays as `null` with an explanatory note -- as done in the initial run.

## Comparative Benchmark Arm

This section documents the `raw_baseline` arm added for issue [#1122](https://github.com/clouatre-labs/aptu/issues/1122).

### Purpose

Establish a head-to-head comparison between `aptu+mercury-2` (structured, schema-enforced) and a raw `claude-opus-4.6` call with a minimal generic prompt (no schema, no rubric, no AST context).

### Procedure

For each of the 6 fixtures (triage: #737, #850, #1094; pr_review: #1101, #1098, #1091):

1. Fetch the issue body or PR diff using `gh issue view` / `gh pr diff`.
2. Truncate input to 8,000 characters.
3. POST to `https://openrouter.ai/api/v1/chat/completions` with model `anthropic/claude-opus-4.6`.
4. Triage prompt: `"Triage this issue. Here is the body: <body>"`
5. PR review prompt: `"Review this PR for issues. Here is the diff: <diff>"`
6. Record wall-clock latency (`time.time()` before/after `urlopen`).
7. Record cost from `usage.cost_details.upstream_inference_cost` (the top-level `cost` field is 0 in OpenRouter BYOK mode).

### Scoring

Apply the rubric in `bench/rubric.md` (C1-C5 binary criteria). C5=0 for all raw_baseline entries by design: the raw prompt enforces no output schema, so JSON conformance cannot be verified.

For PR review C3 (verdict match), a response that only lists issues without an explicit approve/request-changes verdict is scored 0.

### Data Files

- Efficiency data (cost, latency): `bench/results/efficiency.json`
- Quality scores: `bench/results/scores.json` key `raw_baseline`

## Fixture Caveats

- **#800** is a pull request, not an issue; replaced with **#1094** for triage scoring.
- **#737** is a closed/wontfix issue with a self-contained body. C3 (clarifying questions) and C4 (implementation_approach) legitimately score 0 for this fixture class; 3/5 is the expected result.
- **Before scores** are unavailable: pre-#1103 prompts no longer exist in the worktree; only after scores were recorded.
