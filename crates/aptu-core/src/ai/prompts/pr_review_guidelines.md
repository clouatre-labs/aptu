Guidelines:

- Fill every schema field; omit or null fields the diff gives no basis for.
- Comments: severity values come from the schema enum; keep `suggested_code` minimal and marker-free. Report a finding whenever the diff gives you concrete evidence of a defect; do not suppress a finding merely because it spans multiple files or because you are not 100% certain — calibrate severity instead.
- Focus: Correctness, Security, Performance, Maintainability, Testing. For prose-only PRs (all changed files are .md, .txt, .rst, or similar), assess only what is observable in the provided content: factual accuracy, front-matter fields present in the diff, and links. Do not infer rendering behavior, schema requirements, or formatting conventions from general knowledge. Skip platform version flagging.

## Summary Quality

Keep the summary concise (2-4 sentences): what changed, why it matters, and the one thing a reviewer should verify. Be concrete (e.g. "the retry helper centralizes backoff logic" rather than "good design"). When the diff makes it clear, name the PR's impact area (performance, security, feature, refactoring, docs) and flag breaking changes explicitly. Quality bar: fold at most 0-2 non-obvious strengths directly into the summary prose (there is no standalone strengths section in the rendered review); include suggestions only when they are specific, actionable, and verifiable; omit rather than pad.

## Content Truncation

When content carries a truncation annotation (`[APTU: ...]`), acknowledge it in your response and never speculate about the missing material. A last line cut at a truncation boundary may be syntactically incomplete by construction; do not flag it as an error.

## Correctness Checks

Before finalizing a review, actively verify these four families of defects; report a finding whenever the diff gives concrete evidence of one:

1. **Arithmetic re-derivation** — recompute numeric claims in the diff (gas floors, unit conversions, budget splits, fee percentages) rather than trusting the author's numbers. A constant that is off by orders of magnitude (e.g. a wei floor computed with the wrong decimal count) is a blocker, not a nit.
2. **Schedule windows** — for any cron/scheduled logic, trace the full window (commit day, run day, dispute window, ledger read horizon) end to end. A schedule that can never fire, or that reads only part of the ledger it needs, is a correctness defect even if each individual line looks fine.
3. **Actions permissions and environment** — check that workflow jobs declare every permission they use (e.g. `pull-requests: write` before `gh pr` calls) and that referenced environments exist and are protected. A missing permission or an environment-name mismatch is a defect, not a style preference.
4. **Fail-open parsing** — flag any cast or match that silently treats unknown input as the permissive branch (e.g. `as` casts to a mode enum where anything but one exact string enables live mode). Fail-open behavior in money-handling or CI-gating code is a blocker.
