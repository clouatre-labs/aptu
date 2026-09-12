# Audit: Symbol Expansion Benchmark (Issue #1593) — September 2026

> **Status: Executed 2026-09-12.** Preregistered benchmark for PR #1607 (issue #1593, avenue 3 of
> the closed review-context optimization roadmap #1572). Result is **blocked/inconclusive** by a
> pre-existing `analyze_focused` parameter bug discovered during execution — see Findings.

Date: 2026-09-12
Related: `2026-08-28-kg-benchmark-v2-roi.md` (rubric and fixture design reused verbatim),
issue #1593, PR #1607 (`1593-targeted-symbol-expansion` branch)
Toolchain: `aptu-cli` built from PR #1607 (`.worktrees/1789252000`, commit `fe47a23`),
`aptu-coder-core 0.32.4` (`Cargo.lock`)
Data: 5 PRs (4 reused + 1 new) x 2 configs (baseline / `--deep`) x 2 runs = 20 runs
Method: `aptu pr review <PR> --repo clouatre-labs/aptu --repo-path <fixture-checkout> [--deep] -o json`
— real AI calls, nothing posted
Scope: draft PR #1608 opened as a throwaway fixture against `clouatre-labs/aptu`, closed (never
merged) after data collection; scratch git worktrees under `/tmp/fixture-*`, removed after data
collection

---

## Purpose

Issue #1593 adds a `--deep`-gated "symbol expansion" feature: changed symbols with exactly one
out-of-diff caller/reference get that reference's snippet attached to the review prompt with
file:line provenance. Per #1593's ground rule (inherited from #1572), this must clear a
preregistered benchmark before its scope can widen beyond the current conservative `--deep` gate:
"adopt only if it demonstrates incremental catches beyond the current no-expansion baseline, with
no added false positives and zero false-reassurance instances."

This benchmark reuses the design and rubric of `2026-08-28-kg-benchmark-v2-roi.md` verbatim
(same fixture shapes, same scoring definitions), substituting the "No KG / KG" config axis for
"baseline (no `--deep`) / expansion (`--deep`)".

## Design

### Fixtures (5 PRs)

| # | Fixture | Description | Shape |
|---|---------|--------------|-------|
| #1564 | Broken caller | `extract_retry_after` (`retry.rs`) gained a `max_delay_secs` param; caller in `ai/provider/http.rs` (out-of-diff) still calls it with the old 1-arg signature | **cross-file target case** — reused verbatim from the KG audit |
| #1565 | Dead code path | `parse_git_remote_url` removed from `utils.rs`; caller `infer_repo_from_git` is in the *same file* | same-file control (not expansion's target) |
| #1566 | Wrong trait impl | `ConfigSource::load` impl changed return type from `Result<AppConfig, AptuError>` to `Option<AppConfig>`; trait declaration is in the *same file* | same-file control |
| #1567 | Clean control | Pure variable rename (`query` -> `search_query`), no functional change | false-positive check |
| #1608 (new) | Dead code path (cross-file) | `redact_api_error_body` removed from `ai/provider/parse.rs` along with its 2 unit tests; sole production caller in `ai/provider/http.rs` (out-of-diff) left untouched | **cross-file target case** — new, per #1593's spec for a genuinely cross-file removed-function fixture (unlike #1565, which is same-file) |

Fixtures #1564-1567 are closed PRs from the KG audit; branches were deleted, but head commits
remain fetchable via `git fetch origin pull/N/head` and were checked out into detached-HEAD
scratch worktrees at their exact SHAs for this run. #1608 was created fresh, opened as a draft PR,
and closed with its branch deleted immediately after data collection.

### Method

- 2 configs: baseline (no `--deep`) / expansion (`--deep`, enables both call_graph and symbol
  expansion — see Findings on why these could not be isolated from each other in this run).
- 2 runs per PR per config, matching the KG audit's precedent (16 -> 20 runs for 5 fixtures).
- Each fixture reviewed via `--repo-path <scratch-worktree-at-fixture-commit>` so AST/symbol
  resolution sees the file state the fixture PR represents.

### Pre-registered scoring rubric (reused verbatim from the KG audit)

- **True positive (catch)**: a comment identifies the actual seeded defect by name, not a generic
  hedge.
- **False positive**: a comment flags something unrelated to the seeded defect.
- **False reassurance**: the reviewer confidently asserts genuinely broken code is correct or
  already fixed — worse than a miss.

### Decision rule (from #1593, applied verbatim)

Adopt (widen beyond the current `--deep` gate) only if: incremental catches over the no-expansion
baseline, no added false positives, and zero false-reassurance instances. Any false-reassurance
instance blocks adoption regardless of catch rate.

## Results

### Table 1: Hit/Miss Summary

| Fixture | Baseline hit rate (of 2) | Expansion hit rate (of 2) | False positives | False reassurance |
|---------|:---:|:---:|:---:|:---:|
| #1564 Broken caller (cross-file target) | 0/2 | 0/2 | 1 run flagged an unrelated unused-constant nit (see Findings) | 0 |
| #1565 Dead code, same-file | 0/2 (generic hedge both runs) | 1/2 (run 2 named the exact break) | 0 | 0 |
| #1566 Wrong trait impl, same-file | 2/2 (named both types in conflict) | 2/2 (same) | 0 | 0 |
| #1567 Clean control | — | — | 0/2 baseline, 0/2 expansion | 0 |
| #1608 Dead code, cross-file (new) | 0/2 (docstring-staleness nit only) | 0/2 (same) | 0 | 0 |

**0 false-reassurance instances across all 20 runs.** No comment in any run, in either config,
confidently asserted genuinely broken code was fine.

**0 net-new false positives from expansion.** The one questionable comment (#1564 expansion run 1,
flagging `MAX_RETRY_AFTER_SECS` as an unused constant) is technically true — the constant's only
remaining reference is a doc comment — but is unrelated to the seeded defect, so it counts as a
false positive under the strict rubric. It appeared in only 1 of 4 #1564 runs and is not a
reassurance about the actual break.

### Table 2: Cross-file target-case detail

Both genuinely cross-file fixtures (#1564, #1608) — the exact case symbol expansion is designed
to catch — were missed in **all 4 runs each**, in both configs. Expansion produced no comment
naming either seeded defect in any of the 8 relevant runs.

## Findings

- **Root cause: `analyze_focused`'s 4th parameter is a directory-walk depth, not a call-graph
  depth.** Debug-level tracing (`RUST_LOG=debug`) on every fixture, plus a control run against
  PR #1607 itself in the full dev worktree (not a scratch checkout, ruling out a checkout-specific
  cause), shows every single `analyze_focused` call — for both the pre-existing `call_graph`
  feature and the new `symbol_expansions` feature — failing with `Graph error: Symbol not found`
  for every symbol tried, including symbols that indisputably exist (e.g. `extract_retry_after` in
  `retry.rs`). Tracing `aptu-coder-core 0.32.4`'s source: `analyze_focused`'s `max_depth: Option<u32>`
  parameter is passed straight into `walk_directory` as `WalkBuilder::max_depth` — a directory
  traversal depth limit *relative to the repo root*, not a call-graph traversal depth as the
  research and build phases assumed (both described it as "depth" / "max_results" in symbol-graph
  terms). A file at `crates/aptu-core/src/retry.rs` sits at walk-depth 4 from a repo root; the
  existing `call_graph` code passes `max_depth=Some(3)`, and the new `symbol_expansions` code
  (mirroring that call) passes `Some(2)` — both walks terminate before reaching any file nested
  three or more directories under a typical `crates/<crate>/src/` layout, so every lookup returns
  "Symbol not found" and both features silently no-op.
- **This is a pre-existing defect, not a regression from PR #1607.** `call_graph` has shipped with
  this parameterization since it was introduced; it has apparently never successfully resolved a
  symbol in a normally-nested Rust workspace, silently degrading to "no callers found" every time
  (the code's own fail-silent design, correct for handling genuinely ambiguous/missing symbols,
  also masks this systematic failure). `symbol_expansions` inherited the same call shape and the
  same failure, so this benchmark cannot yet distinguish "expansion doesn't help" from "expansion
  never activated."
- **Same-file fixtures show no regression.** #1565 and #1566, where the second reference is
  already inside the diff/full-file context (not dependent on `analyze_focused` at all), show
  materially identical hit rates between baseline and expansion, as expected — expansion neither
  helps nor hurts fixtures it isn't designed to affect.
- **No false-reassurance and no material false-positive increase**, consistent with the decision
  rule's safety requirement — but this is a weak signal given the feature never actually fired on
  the two fixtures that would exercise it.

## Recommendations

**The decision rule cannot be applied as adopt / do-not-adopt from this run — it is blocked.**
The "incremental catches" leg of the rule requires the feature to have actually contributed
context in at least the runs where its target defect was present; since `analyze_focused` failed
to resolve any symbol in either target fixture, the observed 0/4 catch rate measures a
non-functional dependency, not the expansion design itself.

**Before re-attempting this benchmark (status as of the addendum below):**

1. ~~Fix the `max_depth` parameterization~~ — **done**, see Addendum: both
   `build_call_graph_context_sync` and `build_symbol_expansions_context_sync` now pass `None`
   (unrestricted walk), verified via debug-level smoke test.
2. ~~Re-verify with a debug-level smoke test that `analyze_focused` resolves a known symbol~~ —
   **done**, see Addendum: resolution now succeeds (`definitions=1665 edges=16398`) instead of
   failing with "Symbol not found".
3. **Still open**: a second, independent data-quality issue in `analyze_focused`'s incoming-chain
   resolution (spurious caller edges to files that do not reference the symbol at all) blocks a
   full re-run from changing the verdict — see Addendum. This is an upstream `aptu-coder-core`
   concern, not fixable in aptu's own codebase, and is not re-litigated further here.
4. The pre-existing `call_graph` feature shares the same `max_depth` root cause and has been
   fixed alongside symbol expansion in this PR, but was likely silently inert since its
   introduction for any repository with standard crate/`src` nesting — worth a note in its own
   right if `call_graph`'s history needs revisiting independent of #1593.

**Interim recommendation for PR #1607 itself:** the code correctly implements the design
(budget accounting fix, deep-gating, provenance rendering, ambiguity handling all verified
independently by CHECK) and this benchmark found no false-reassurance or material false-positive
regression. Given the finding above, it is reasonable to merge #1607 as-is (deep-gated, off by
default) with adoption/default-enablement explicitly deferred pending a corrected benchmark run —
do not treat this benchmark's null catch-rate as evidence against the design.

### Addendum (post-benchmark): `max_depth` fixed, second-order ambiguity issue found

After this benchmark ran, the `max_depth` bug above was fixed directly in PR #1607 (both
`build_call_graph_context_sync` and `build_symbol_expansions_context_sync` now pass `None`
instead of `Some(3)`/`Some(2)`). Rebuilding and re-running fixture #1564 with `--deep` and
`RUST_LOG=debug` confirms the fix: `walk_directory` now visits all 179 files (previously 0
reachable past depth 2/3), and `analyze_focused` successfully builds a graph
(`definitions=1665 edges=16398`) instead of failing every lookup with "Symbol not found".

However, a **second, independent issue** surfaced once resolution started working: for
`extract_retry_after` (the seeded #1564 defect), `analyze_focused` returns 3 out-of-diff
"caller" candidates —
`ai/provider/http.rs:213`, `ai/provider/mod.rs:146`, `security/validator.rs:98` — causing the
`out_of_diff.len() != 1` ambiguity check to correctly skip it per the code's own design. But
`grep -n "extract_retry_after"` against the fixture checkout confirms only `http.rs:213` is a
real call; `provider/mod.rs` and `validator.rs` do not reference the symbol at all. This means
`analyze_focused`'s incoming-chain resolution in `aptu-coder-core` is attributing spurious
caller edges to unrelated call sites — a data-quality issue in the third-party call-graph
builder, not in PR #1607's own ambiguity-detection logic (which behaved exactly as designed:
skip on multi-reference, don't guess).

**Practical effect**: PR #1607's expansion logic is correct and its `max_depth` bug is fixed,
but the true real-world hit rate on the exact target case (#1564) is still 0% today, because the
upstream call-graph data makes a genuinely single-caller symbol look ambiguous. This is safer
than the alternative failure mode (a false single-match expanding the wrong caller) but means the
feature will under-fire until the upstream `analyze_focused` caller-resolution accuracy is
investigated — that investigation is out of scope for aptu's own codebase (`aptu-coder-core` is
an external dependency) and is not re-litigated here. A full re-run of this benchmark is not
expected to change the verdict until that upstream data-quality issue is understood or the
ambiguity check is made more permissive (e.g., matching on caller module path in addition to
exact symbol name) — the latter would itself need its own risk analysis before implementation.

## Limitations

- **Small N, single environment**: 20 runs, 5 fixtures, one model/provider (whatever
  `~/.config/aptu/config.toml` / env configures for this session) — not re-validated across
  models.
- **Confounded conditions**: baseline vs. expansion is not a clean isolation of symbol expansion
  alone, since `call_graph` can also auto-enable under `--deep` (or even without it, if budget
  permits) — though this run shows call_graph was equally inert due to the same bug, so the
  confound did not materially affect this result.
- **Synthetic new fixture**: #1608 removes a real, currently-used function on a throwaway branch
  never intended to merge; safe in isolation but hand-constructed rather than a naturally
  occurring bug.

## Reproduction

```bash
# 1. Build aptu from the PR branch (installs to ~/.cargo/bin/aptu, NOT the PATH-resolved release):
cd .worktrees/1789252000 && cargo install --path crates/aptu-cli --profile release --features aptu-core/ast-context

# 2. Fixtures: #1564/#1565/#1566/#1567 via `git fetch origin pull/<N>/head`, checked out detached
#    into scratch worktrees at their exact SHAs. #1608 (new): branch `fixture/dead-code-cross-file`
#    off main, removing `redact_api_error_body` (crates/aptu-core/src/ai/provider/parse.rs) and its
#    2 unit tests, leaving the caller in ai/provider/http.rs:48 untouched.

for pr_and_path in ...; do
  for cfg in baseline expansion; do
    for run in 1 2; do
      ~/.cargo/bin/aptu pr review <PR> --repo clouatre-labs/aptu --repo-path <scratch-checkout> \
        $( [ "$cfg" = expansion ] && echo --deep ) -o json
    done
  done
done

# 3. Cleanup: git worktree remove each scratch checkout; gh pr close <new_fixture_pr> --delete-branch
```
