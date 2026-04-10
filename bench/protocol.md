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
