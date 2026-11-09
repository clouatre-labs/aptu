# Audit: AI review approved a PR with 6 serious blockers

- **Date:** 2026-11-09
- **Scope:** `aptu` review pipeline, `aptu-github-app` trigger/model configuration
- **Trigger:** A PR in a private repository (~1,275 added lines, 12 files, cron-triggered crypto payout workflow) received an "✅ Approved" review from the aptu bot with zero substantive findings. A human self-review found 6 blockers and 5 workflow-level defects; the bot's second review (of the fix commit) still approved and missed 4 remaining blockers. All human findings were verified real and subsequently fixed.

## Missed findings (from the human review)

Round 1 (first commit):

1. Missing payout-address registry — every partner skips `no_registered_payout_address`; the CLI can never produce a payable plan in any mode.
2. Retry double-pays — reconcile-before-re-sign stubbed to `null`; statements keep `<pending>` forever; prior run records never read back.
3. Cron can never pay — day-1 statement commit + day-2 run + 7-day dispute window + month-only ledger read ⇒ every partner skips `dispute_window_open` every month.
4. Environment mismatch — docs gate live mode behind a protected `disbursement` environment; workflow references `environment: production`.
5. ETH gas floor ~8 orders of magnitude too low — `toUnits` pads the fraction to 6 digits regardless of decimals (5,000,000 wei instead of 5×10¹⁴).
6. Fail-open mode parsing — `as "dry-run" | "live"` cast treats any value other than exactly `"dry-run"` as live.

Round 2 (fix commit), still missed by the bot:

7. Retry still double-pays (stub untouched — invisible in the delta diff).
8. Partial-failure run records never committed (success-gated commit step).
9. `gh pr create` 403 — missing `pull-requests: write`.
10. Default monthly dry-run fails at `git add` on an empty directory.

## Root causes (verified in code)

### 1. Model depth: mid-tier MoE, unmeasured for this task

The reviews ran on `google/gemma-4-26b-a4b-it` — a real model (Gemma 4 26B A4B IT, April 2026, MoE with 25.2B total / 3.8B active params, $0.04/$0.15 per M tokens, Artificial Analysis Coding Index 39.3). It was explicitly pinned in the repository's `.github/aptu.yml`, a default propagated by the aptu-github-app welcome-issue template (`worker/src/index.ts:218`); the workflow fallback (`aptu-review.yml:28`) uses the same model. Bugs 3, 5, 6, 9 require arithmetic re-derivation, cross-hunk logic tracing, or Actions permission knowledge — capabilities the pipeline gave the model no instruction to apply (causes 3 and 7) and that a 3.8B-active model is not demonstrably strong at. Whether a stronger model would have caught them is unmeasured; the recommendation below is therefore benchmark-driven, not a blind swap.

### 2. Diff-only context: full file contents disabled and deprioritized

- The app passes `max-full-content-files: "0"` (`.github/workflows/pr-review.yml:86`), so the model never sees any full file — only hunks.
- Even at the default (10 files), the prompt budget drops full file contents largest-first first (`ai/review_context/budget.rs:68-95`), so on a PR this size they would be evicted anyway.
- Consequence: bug 1 (a registry that does not exist anywhere) is undetectable by construction, and the model cannot trace what a changed function's callers or imports actually do.

### 3. Prompt guideline bias toward empty findings

`ai/prompts/pr_review_guidelines.md:4` literally instructs: "use an empty comments array when uncertain or multi-file". A 12-file PR is exactly the case this suppresses. There are no instructions about verifying arithmetic, tracing control flow, checking Actions permissions, or treating cron/scheduling as a correctness surface.

### 4. Verdict is decorative; approval has no consequence

The model's `verdict` string is rendered as a text badge only (`ai/triage.rs:631-633`). The GitHub review event comes purely from the CLI flag — the action always runs `aptu pr review --comment` (`action.yml:389`), so `request_changes` from the model produces a COMMENT event that still reads "✅ Approved" when the model says approve. Nothing forces the conclusion to be consistent with the findings.

### 5. Verification pass can only delete findings

`filter_verified_findings` (`ai/provider/review.rs:232-246`) is fail-closed: findings without a matching verify verdict are dropped (`min_confidence: 0.7`). The verifier sees only the truncated diff and can never add, escalate, or flip the verdict. Combined with cause 3, the pipeline's only pressure is toward fewer findings and an approving tone; nothing generates the "approve is wrong" signal.

### 6. Incremental review hides previously-seen code

When a prior `APTU_REVIEW:<sha>:<hash>` marker exists, only the `last_sha..head` delta is fetched and prior findings are injected with "do not repeat these findings" (`ai/analyze.rs:81-118`, `prompts/mod.rs:473-480`). Round 2 was delta-only (+381/−44). The round-1 defects that the fix commit did not touch (bug 7) were invisible. Worse: a prior review with zero findings means nothing was "confirmed" — yet the delta-only mode still applies.

### 7. No workflow-file awareness or repo context

Workflow YAML is treated as an ordinary patch. The review prompt has no CI-status input, no repo tree, and no instruction that `.github/workflows/*` deserve permission, environment, concurrency, and schedule-window checks.

## Recommended fixes (validated for scope)

High leverage, minimal surface:

1. **Rewrite the guideline bias** (`pr_review_guidelines.md`): remove "empty comments when uncertain"; add explicit calibration guidance and correctness checks (arithmetic, schedule windows, Actions permissions/env, fail-open parsing).
2. **Make the verdict consequential**: when the model returns `request_changes` (or blocker-level concerns exist), the posted review must not read "Approved"; wire model verdict → review event instead of CLI-flag-only.
3. **Verifier escalation**: let the verify pass confirm *or* escalate; stop silently dropping unverified findings when the initial finding cites concrete evidence; allow the verifier to flip an approve verdict.
4. **Incremental-review guard**: if the prior review contained zero (or only withdrawn) findings, review the full diff, not the delta; always include prior blockers that are not visibly fixed.
5. **App-side model pairing**: propose `deepseek/deepseek-v4.1-flash-1017` as primary (equal-or-cheaper at review-prompt scale, stronger coding benchmarks) with the incumbent `google/gemma-4-26b-a4b-it` demoted to the existing fallback slot (replacing `inception/mercury-2`); aptu's `FallbackConfig` chain already supports this with no code change. Confirm via the fixture eval and revert if the challenger underperforms (issue clouatre-labs/aptu-github-app#337).
6. **Stop passing `max-full-content-files: "0"`** in the app; restore the aptu default (10) so hunks have surrounding context (same issue).

Deferred (YAGNI for now): repo-tree injection, import-graph fetching, risk-based model routing classifier.

## Lessons

- Truncation was **not** the cause: every budget had ample headroom.
- The failure is systemic suppression: empty-findings guidance + delete-only verification + consequence-free verdict, on a model tier too shallow to re-derive arithmetic and scheduling logic from hunks alone.
- Human review remains required for money-handling code paths until at least fixes 1–4 land.
