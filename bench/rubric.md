# Evaluation Rubric

Binary scoring is used for each criterion: **1** = pass, **0** = fail.

## Triage

| Criterion | Description |
|-----------|-------------|
| C1 | Primary label matches the expected label for the fixture |
| C2 | Summary contains no claims absent from the issue text |
| C3 | At least one clarifying question is non‑trivially answerable from the issue body |
| C4 | `implementation_approach` mentions at least one specific file or function |
| C5 | Output is valid JSON conforming to the triage schema |

## PR Review

| Criterion | Description |
|-----------|-------------|
| C1 | At least one comment references a specific file and line number |
| C2 | No hallucinated file path or line reference |
| C3 | Verdict (approve / request‑changes / comment) matches the known outcome |
| C4 | At least one comment contains an actionable suggestion, not only an observation |
| C5 | Output is valid JSON conforming to the PR review schema |

**Score formula**: Sum the passed criteria (0‑5) for each fixture. The overall pass threshold is **≥4/5** on all fixtures in both groups.
