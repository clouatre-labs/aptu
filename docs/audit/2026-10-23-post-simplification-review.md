# Audit: Post-Simplification Review — October 2026

Date: 2026-10-23  
Data: `~/.pi/agent/sessions` (command-frequency extraction across all session JSONL), `git log` (commits #1760-#1775), aptu source at `49be0cf`, aptu-github-app at `ce8a63f`, GitHub issue search, prior audit `2026-10-21-product-scope-context-robustness.md`  
Issues: aptu #1777, #1778, #1779, #1780, #1781, #1782, #1783, #1784, #1785; aptu-github-app #311, #312, #313, #314, #315  
Method: two parallel read-only scouts (aptu CLI/core inventory, app + CI review flow), followed by two adversarial validators (coder-guard) that traced every proposed retirement against all entrypoints (CLI, `action.yml`, `.github/workflows/`, dispatch types, tests, docs) and re-verified each claimed bug against HEAD. All file/line references below were confirmed at current HEAD.

## Purpose

Validate that the recent simplification pass (#1760-#1769, #1770-#1773) landed cleanly, identify further retirement/refactor opportunities with regression analysis, and confirm or refute suspected defects in the review path. Goal: make the two key features (aptu-dev via the GitHub App, `aptu pr review` in CI and CLI) fully optimal.

## Usage reality

*Table 1: aptu command frequency in pi agent sessions (`~/.pi/agent/sessions`, extraction over all JSONL).*

| Command | Observed executions | Share |
|---|---|---|
| `aptu pr review <url> -o json` | ~3,875 | ~59% |
| `aptu scan-security --diff` | ~1,600 | ~25% |
| `aptu lint-issue` (references; app-driven) | ~550 | ~8% |
| `aptu triage` (executed locally) | ~2 | <1% |
| `aptu history` / `auth` / `config` (executed) | ~2 | <1% |

`~/.local/share/aptu/history.json` does not exist: contribution recording remains unexercised. The prior audit's finding stands; its removal landed in #1769.

## Findings

### F1 — Remaining retirement surface, regression-checked

**Severity:** Medium (code health)  
**Finding:** Seven retirements survived adversarial validation; three did not survive as originally scoped. Safe (remove action surface in the same change): `pr label` (#1777), bulk triage `--since/--state` (#1778), AST/call-graph + `--repo-path` (#1780), circuit breaker (#1781, requires unwiring `client.rs:136,193` and `http.rs:295,400`), FindingCache only (#1779 — the generic `FileCache` in `core/cache.rs` is load-bearing for `resolve_context_window`), `facade/ai_client.rs` merge (#1783). Structural: replace `ai/registry/parsing.rs` (501 lines of model catalog) with config-declared context windows (#1782, highest effort — budget chain must keep working with zero config). Rejected by validation: metrics JSONL writers (they feed the action's `APTU_CONTEXT_FILE` rollup) and `prior_findings_summary` unboundedness (it is already budget-bounded via `PRIOR_FINDINGS_MAX_CHARS`, `review_context/mod.rs:200-220`).  
**Issues:** #1777, #1778, #1779, #1780, #1781, #1782, #1783

### F2 — Review-path defects confirmed at HEAD

**Severity:** High (cost), Low (correctness)  
**Finding:** Two defects confirmed. (1) `fetch_pr_files_since` returns `None` for both an empty compare range and compare-API errors, so a `synchronize` push with no new diff runs a full fetch + full AI review whose output is discarded at posting time (`post.rs:335`) — a wasted Actions job and AI spend per occurrence (#1784). (2) The 200 KB cap in `reconstruct_diff_from_pr` (`analyze.rs:173-181`) silently excludes files from finding verification while they are still reviewed; no log, no telemetry (#1785). The stale-comment dedup collision on `(path, original_line, side)` was confirmed but is a documented tradeoff, not filed.  
**Issues:** #1784, #1785

### F3 — App-side: dead analytics, live draft-gate bug, dead lint flow

**Severity:** Medium  
**Finding:** The `INSTALLATION_USAGE` Durable Object (#308) has zero readers (no route, cron, or export) — safe to remove with a `deleted_classes` migration (#313). Draft PRs are gated only on `opened` (`worker/src/index.ts:1118`); a push to an open draft still dispatches a full review (#311). The `aptu-lint-issue` flow is a silent no-op since the aptu CLI command was removed in #1769 (`lint-issue.yml:141-144` gracefully skips) — retire end-to-end (#314). The `/telemetry/rollup` aggregate is also a write-only sink, but it has live opted-in senders, so it needs an explicit expose-or-retire decision rather than deletion (#315). Welcome issue and Sentry: keep and defer respectively (Sentry is deeply wired; removal is a migration, not cleanup).  
**Issues:** #311, #313, #314, #315 (aptu-github-app)

### F4 — PR review in CI: biggest remaining wins

**Severity:** High (cost/quality)  
**Finding:** Two complementary fixes eliminate wasted review runs. CLI-side, the empty-range short-circuit (#1784) stops aptu itself from reviewing nothing. App-side, the worker can read the last bot marker SHA via `GET /repos/{o}/{r}/pulls/{n}/reviews` before dispatching and skip unchanged-head `synchronize` events entirely (#312) — one extra API call versus one Actions job plus AI tokens. Longer term, editing a single sticky review comment in place per PR (the marker design already anchors it) would remove duplicate-comment noise entirely; not yet filed pending design.

## Issue map

*Table 2: Filed issues by repo, type, and theme.*

| Issue | Repo | Type | Theme |
|---|---|---|---|
| #1777 | aptu | refactor | Retire `pr label` + action step |
| #1778 | aptu | refactor | Retire bulk triage surface |
| #1779 | aptu | refactor | Retire FindingCache (keep FileCache) |
| #1780 | aptu | refactor | Retire AST/call-graph + `--repo-path` |
| #1781 | aptu | refactor | Remove circuit breaker, keep retry |
| #1782 | aptu | refactor | Config-declared context windows |
| #1783 | aptu | refactor | Merge facade/ai_client into ai/client |
| #1784 | aptu | fix | Short-circuit empty incremental range |
| #1785 | aptu | fix | Surface partial verification coverage |
| #311 | aptu-github-app | fix | Draft gate on synchronize |
| #312 | aptu-github-app | feat | Worker head-SHA short-circuit |
| #313 | aptu-github-app | chore | Remove INSTALLATION_USAGE DO |
| #314 | aptu-github-app | chore | Retire aptu-lint-issue flow |
| #315 | aptu-github-app | chore | Telemetry: expose or retire |

## Suggested sequencing

1. #1784 + #312 (cost bugs, independent) and #311 (trivial gate fix).
2. Pure retirements: #1777, #1778, #1779, #1781, #1783, then #1780 (largest deletion).
3. App cleanups: #313, #314, decision on #315.
4. Structural: #1782 (config-declared context windows), #1785 (verification coverage telemetry).
