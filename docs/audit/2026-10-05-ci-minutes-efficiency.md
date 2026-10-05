# Audit: GitHub Actions Minutes and Wall-Clock Efficiency - October 2026

Date: 2026-10-05
Data: `gh run list` and `gh api repos/clouatre-labs/aptu/actions/runs/{id}/jobs` over the trailing 7 days (15 runs per workflow), `.github/workflows/*.yml`, branch rulesets 13365825 and 11104020
PRs: [#1737](https://github.com/clouatre-labs/aptu/pull/1737)

## Purpose

Point-in-time audit of GitHub Actions consumption in `clouatre-labs/aptu`. The repository is public, so standard-runner minutes are free; the optimization targets are wasted compute and wall-clock, not billed cost. Security posture and merge-gate guarantees must remain identical.

## Baseline (trailing 7 days)

| Workflow | Runs/week | Mean job duration | Compute-min/week |
|---|---|---|---|
| CI - Test | ~10 | 215s | ~36 |
| CI - Check Format & Lint | ~10 | 188s | ~31 |
| CI - Build CLI | ~10 | 100s | ~17 |
| CI - GraphQL Contract (non-blocking) | ~10 | 65s | ~11 |
| Triage Pull Requests (changes + review) | 14 | 28s | ~8 |
| Security | 22 | 15s | ~6 |
| OpenSSF Scorecard | 9 | 29s | ~4.5 |
| CI - WASM/Scan Self/Deny/Lint Commits/jscpd/Lint Docs | ~10 | <30s each | ~10 |
| REUSE Compliance | 10 | 12s | ~2 |
| Scheduled Security Audit | 1 | 21s | ~0.4 |

Total: roughly 130 compute-minutes per week. Wall-clock is about 4 minutes per PR and 4.5 minutes per push to main.

## Findings

| # | Category | Severity | Finding | Action | PR |
|---|---|---|---|---|---|
| 1 | Redundant runs | Low | Scorecard ran on every push to main (8 runs/week, 29s each) in addition to the weekly schedule; not a merge gate | Remove `push` trigger; keep weekly schedule and `workflow_dispatch` | #1737 |
| 2 | Advisory signal | Low | GraphQL Contract job costs ~11 min/week but is `continue-on-error`, so it never gates merges | Left alone: inline comment documents it as intentional live-API signal | - |
| 3 | Post-merge validation | Low | Full CI pipeline reruns on main for every merged Renovate bump (8 of the last 10 pushes to main) | Left alone: intentional trunk validation; `save-if: refs/heads/main` cache writes depend on these runs | - |
| 4 | Billing idle | Low | Release workflow has a `sleep 30` step (crates.io index propagation) | Left alone: required for publish correctness; release events are rare | - |

## Accepted trade-off

The Scorecard badge and SARIF upload now refresh weekly (Mondays 06:00 UTC) instead of on every push to main. `workflow_dispatch` remains available for ad-hoc refresh. No merge-gate impact: the required checks are `CI Result` (the always-runs aggregate job) and `DCO`.

## Already optimal (verified, no change)

- All jobs run on `ubuntu-26.04-arm`.
- `ci.yml` uses a `changes` job with `dorny/paths-filter` and per-job `needs.changes.outputs` gates, with an `if: always()` `ci-result` aggregate as the sole ruleset-required check; no trigger-level `paths:` filter can starve it. REUSE Compliance has `paths:` filters but is not a required check, so no starvation risk.
- Concurrency groups with `cancel-in-progress` on CI, Security, and REUSE; serial where cancellation would leave work half-done (Triage Pull Requests, Release).
- `renovate[bot]` actor gates skip expensive jobs on dependency PRs.
- Shared Swatinem rust-cache with `save-if` restricted to main.
- All actions pinned to full commit SHAs with version comments; top-level `permissions: {}` with minimal per-job grants; zizmor clean.
