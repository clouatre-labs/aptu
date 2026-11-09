# Fixture: review-miss-2026-11-09

- **Audit:** [docs/audit/2026-11-09-review-miss-root-cause.md](../../2026-11-09-review-miss-root-cause.md)
- **Issue:** clouatre-labs/aptu#1801
- **Purpose:** a self-contained fixture that reproduces the review miss the audit documents: a mid-tier model approved a payout workflow containing six real defects with zero findings.

## Contents

- `disburse.ts` — a cron-triggered crypto payout workflow (TypeScript) that deliberately contains one instance of each defect family below.

## Defect families exercised

1. **Arithmetic re-derivation** — `toWei` pads a basis-point share to 6 decimal digits instead of 18, producing a wei floor ~8 orders of magnitude too low.
2. **Schedule windows** — `reconcile` runs only on the 1st of the month and reads the ledger only from the month start, so the 7-day dispute window opened on day 1 is never re-checked on days 2-7.
3. **Actions permissions** — the workflow calls `gh pr comment` without granting `pull-requests: write`, so the step fails with 403 in CI.
4. **Environment mismatch** — the job targets `environment: prod` while the repo defines a protected `production` environment, so the protection rules never apply.
5. **Fail-open parsing** — `parseMode` treats any input other than the exact string `dry-run` as live mode, including typos and untrusted input.
6. **Retry double-pay** — `reconcileBeforeReSign` is stubbed to `null`, so a retried payout is signed and broadcast a second time.

## Expected review outcome

A reviewer applying the `Correctness Checks` section of `crates/aptu-core/src/ai/prompts/pr_review_guidelines.md` should report all six families against this fixture. A model that returns zero findings on it is under-calibrated and should be re-evaluated against the guidelines before merge.
