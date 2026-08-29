# Audit: KG Benchmark v2 — Cost + Value (ROI) — August 2026

> **Status: Executed 2026-08-29.** This doc promotes the "Proposed Benchmark v2" design sketched
> in `2026-08-27-kg-benchmark-baseline.md` into a standalone, execution-ready plan, then executes
> it. All 16 runs completed against real draft PRs (#1564-#1567) on `clouatre-labs/aptu`; the PRs
> were closed (never merged) and the branches deleted after data collection. See Results below.

Date: 2026-08-28
Related: baseline R6/R7 (`2026-08-27-kg-benchmark-baseline.md`), post-consolidation R9
(`2026-08-28-kg-benchmark-post-consolidation.md`)
Toolchain: `aptu 0.10.16` (release build from `main`), `aptu-coder-core 0.32.4` (`Cargo.lock`),
`main`@`fa90f38aa4cde762b2defea11bb9b68b47dd08ae`.
Data (planned): 4 PRs (3 defective + 1 clean control) x 2 configs (No KG / KG) x 2 runs = 16 runs
Method (planned): `aptu pr review <PR> --repo clouatre-labs/aptu -o json` — real AI calls,
nothing posted
Scope: draft PRs on throwaway branches against `clouatre-labs/aptu`, closed (never merged) after
data collection; `~/.config/aptu/config.toml` (`[graph]` section); `~/.local/share/aptu/graph/`
disk cache

---

## Purpose

Every prior KG audit measured **cost** (token overhead, cache behavior, latency) but explicitly
found **no evidence of value** (baseline Limitations; F12). Both the baseline (R6) and the
post-consolidation audit (R9) block KG default-enablement on exactly this gap: does KG help a
reviewer catch structural defects that a diff-only review misses, and if so, is the token
overhead worth paying? This benchmark is designed to answer that question directly, so the
default-enablement decision can be made on evidence instead of deferred again.

This is intentionally the simple version. It is not a rigorous, publication-grade experiment
(no pre-registered stats, no multi-model cross-validation, no human inter-rater reliability) —
see `~/git/clouatre-labs/prompt-repetition-experiments/paper/paper.tex` for what that would look
like. Sixteen runs with binary hit/miss scoring is enough to rule KG in or out for this decision;
if the result is borderline, that itself is useful signal to invest in a more rigorous follow-up.

## Design

Reuses the baseline's already-vetted 3-defect design verbatim (baseline doc, "Proposed
Benchmark v2") rather than inventing new fixtures.

### Fixtures (4 PRs, opened as draft branches against `clouatre-labs/aptu`)

Each fixture: 1-3 files, 10-30 lines. Opened as a real draft PR (the review tool fetches diffs
via the GitHub API, so it must exist on GitHub) on a throwaway branch, never merged, closed
after data collection.

| # | Fixture | Description | Why KG should help |
|---|---------|--------------|---------------------|
| 1 | Broken caller | Change a function signature; leave a caller un-updated | Graph shows the caller edge; reviewer flags the mismatch |
| 2 | Dead code path | Remove a function that is still called elsewhere | Graph shows incoming edges; reviewer flags the dangling call |
| 3 | Wrong trait impl | Implement a trait method with the wrong return type | Graph shows impl relationships; reviewer cross-checks |
| 4 | Clean control | Same file shapes as fixtures 1-3, no bug | Negative control — checks false-positive rate |

**Note before executing:** opening and closing PRs against the real `clouatre-labs/aptu` repo is
a visible GitHub action (even though never merged). Confirm with the repo owner before the
follow-up session opens them, and close all 4 immediately after data collection.

### Method

- 2 configs: No KG (no `[graph]` config) / KG (`[graph] enabled = true`, `max_depth = 4`,
  `max_nodes = 50000`, `cache_ttl_hours = 24` — same values as prior benchmarks).
- 2 runs per PR per config (not 3 — cost metrics are already stable per baseline/post-
  consolidation; value scoring is binary and doesn't need a third run). 4 PRs x 2 configs x 2
  runs = 16 runs.
- Same model as every prior KG benchmark for comparability: OpenRouter `mistralai/mistral-small-2603`.
- Clear the graph cache between configs only, never between runs of the same config (established
  method; run 1 of each config is cold, run 2 is warm — also re-confirms F14's cold/warm parity
  fix holds under real defect content).

### Pre-registered scoring rubric

Decided here, before any run, to avoid scoring the transcripts to fit a preferred conclusion.
For each run, extract the full `review.comments` array text (not just the count) and classify
against the fixture's specific defect:

- **True positive (catch)**: a comment identifies the actual defect (e.g., for fixture 1, a
  comment naming the stale caller and the signature mismatch — not just "check callers").
- **False positive**: a comment flags something unrelated to the seeded defect.
- **False negative**: the defect exists in the diff and no comment addresses it.

Per-PR score is binary: did any comment in that run catch the seeded defect (yes/no)? For the
clean control, "false positive" is the only relevant outcome — a hallucinated defect on a clean
diff.

### Cost metrics (same fields as every prior benchmark)

`input_tokens` (primary metric, per baseline R4), `prompt_chars`, `cost_usd`, `duration_ms`.

### Decision rule

KG is worth proposing for default-enablement if it catches defects that No-KG misses, without
increasing false positives on the clean control. Cost is secondary if the value is real — a
token-overhead percentage alone does not block enablement per this rule.

## Limitations (carried forward, plus new ones for this design)

- **Small N**: 16 runs, 4 fixtures. Binary hit/miss is interpretable at this scale (per baseline),
  but a borderline result (e.g., KG catches 2 of 3 defects, No-KG catches 1 of 3) will not be
  statistically conclusive — treat it as a signal to run more fixtures, not as a tie-breaker.
- **Single model**: `mistral-small-2603` only, for comparability with prior benchmarks. A result
  here does not generalize to other providers/models without re-running.
- **No human inter-rater check**: scoring TP/FP/FN is done by one reader against the rubric above.
  Fine for this simple pass; a rigorous follow-up (see paper reference above) would use 2+ raters.
- **Synthetic fixtures**: hand-written defects, not naturally occurring bugs. They test whether
  KG *can* help on the exact shape of defect it's theoretically suited for — not whether real-
  world PRs contain defects of this shape often enough to matter in practice.

## Results

### Table 1: All 16 Runs

| PR | Config | Run | Prompt Chars | Input Tokens | Cost (USD) | Duration (ms) | Verdict | Defect Caught? |
|----|--------|-----|-------------:|-------------:|-----------:|---------------:|---------|-----------------|
| #1564 Broken caller | No KG | 1 (cold) | 12216 | 4829 | 0.00094335 | 3187 | approve | No |
| #1564 Broken caller | No KG | 2 (warm) | 12216 | 4829 | 0.00027147 | 2867 | approve | No |
| #1564 Broken caller | KG | 1 (cold) | 12864 | 5004 | 0.00033720 | 2741 | approve | No |
| #1564 Broken caller | KG | 2 (warm) | 12864 | 5004 | 0.00023856 | 2864 | approve | No |
| #1565 Dead code path | No KG | 1 (cold) | 18365 | 6083 | 0.00134156 | 2224 | approve | No (wrongly asserted the removed fn is unused) |
| #1565 Dead code path | No KG | 2 (warm) | 18365 | 6071 | 0.00106845 | 2453 | approve | **Yes** (flagged `infer_repo_from_git` still calls the removed fn) |
| #1565 Dead code path | KG | 1 (cold) | 18495 | 6122 | 0.00134813 | 2151 | approve | No (generic "ensure callers updated" hedge, doesn't name the break) |
| #1565 Dead code path | KG | 2 (warm) | 18495 | 6122 | 0.00134813 | 2109 | approve | No (wrongly asserted the caller *was* updated) |
| #1566 Wrong trait impl | No KG | 1 (cold) | 34234 | 9869 | 0.00225544 | 4026 | approve | **Yes** (flagged impl/trait return-type mismatch, "will cause a compilation error") |
| #1566 Wrong trait impl | No KG | 2 (warm) | 34234 | 9857 | 0.00158859 | 14419 | approve | No |
| #1566 Wrong trait impl | KG | 1 (cold) | 35435 | 10175 | 0.00054381 | 3766 | approve | No (called the mismatch "correct and aligns with the intent") |
| #1566 Wrong trait impl | KG | 2 (warm) | 35435 | 10175 | 0.00052461 | 3329 | approve | No |
| #1567 Clean control | No KG | 1 (cold) | 19037 | 6460 | 0.00133425 | 1796 | approve | N/A — 0 comments, no FP |
| #1567 Clean control | No KG | 2 (warm) | 19037 | 6460 | 0.00150450 | 2900 | approve | N/A — 2 style/naming comments, no FP |
| #1567 Clean control | KG | 1 (cold) | 19868 | 6660 | 0.00106944 | 3262 | approve | N/A — 2 style/naming comments, no FP |
| #1567 Clean control | KG | 2 (warm) | 19868 | 6660 | 0.00019428 | 1586 | approve | N/A — 0 comments, no FP |

Every one of the 16 runs returned verdict `approve`; the verdict field carried no signal
independent of the per-comment scoring below.

### Table 2: Hit/Miss Summary (value)

| Fixture | No KG hit rate (of 2 runs) | KG hit rate (of 2 runs) | False positives (No KG / KG) |
|---------|---------------------------|--------------------------|-------------------------------|
| 1: Broken caller | 0/2 | 0/2 | — / — |
| 2: Dead code path | 1/2 | 0/2 | — / — |
| 3: Wrong trait impl | 1/2 | 0/2 | — / — |
| 4: Clean control | — | — | 0/2 / 0/2 |

### Table 3: Cost Summary

| PR | No KG avg tokens | KG avg tokens | Token delta |
|----|-------------------:|----------------:|-------------|
| #1564 Broken caller | 4829 | 5004 | +175 (+3.6%) |
| #1565 Dead code path | 6077 | 6122 | +45 (+0.7%) |
| #1566 Wrong trait impl | 9863 | 10175 | +312 (+3.2%) |
| #1567 Clean control | 6460 | 6660 | +200 (+3.1%) |

## Findings

- **KG caught zero defects that No-KG missed.** On the two fixtures where the seeded defect was
  caught at all (#1565, #1566), it was caught by a No-KG run, never by a KG run. KG's hit rate was
  0/2 on every single fixture, including the two designed specifically to need call-graph/impl-graph
  context (broken caller, wrong trait impl).
- **No-KG caught the two catchable defects via same-file/whole-file context, not a graph.** The
  fixture design assumed a reviewer without a graph "can't tell from the diff alone," but for both
  #1565 and #1566 the relevant second reference (the surviving caller; the trait declaration) sits
  in the *same file*, close enough that ordinary full-file/AST context surfaced it without any
  call-graph or blast-radius rendering. The clean caught cases: No-KG run2 on #1565 named
  `infer_repo_from_git` explicitly as a now-broken caller; No-KG run1 on #1566 named the exact
  trait/impl return-type mismatch and predicted the compile error. Fixture #1564 (the one true
  cross-file case — the caller is in a different file, `ai/provider/http.rs`, entirely absent from
  the diff) was missed by all 4 runs in both configs — this is the fixture that actually tests
  what a call graph is for, and neither config caught it.
- **KG produced confidently wrong reassurance twice, not just silence.** On #1565 run2 (KG), the
  reviewer stated the caller "was updated to use the same parsing logic directly" — false; the
  caller is untouched and now dangling. On #1566 run1 (KG), the reviewer called the trait/impl
  mismatch "correct and aligns with the intent" — actively endorsing a real type mismatch. Both
  are worse than a miss: they're a fabricated justification for why genuinely broken code is fine,
  and both occurred only in the KG condition, never in No-KG on the same fixture/run-parity slot.
- **No false positives on the clean control in either config** — 0/2 hallucinated defects for both
  No-KG and KG. The comments on #1567 were all legitimate (if minor) naming/docstring nitpicks
  about the actual rename diff, not fabricated issues.
- **Cost overhead is small but strictly positive and buys nothing here**: KG added +0.7% to +3.6%
  input tokens on every fixture (Table 3), consistent with prior benchmarks' cost findings, with
  zero offsetting value in this run.

## Recommendations

Applying the decision rule verbatim ("KG is worth proposing for default-enablement if it catches
defects that No-KG misses, without increasing false positives on the clean control"): **the
condition is not met — do not propose KG for default-enablement.** KG did not catch anything
No-KG missed on any of the 4 fixtures; the token-cost delta is therefore paid for zero measured
value, and this run's KG condition twice produced a false "this is fine" claim about genuinely
broken code on the very fixtures the graph is meant to help with. This extends, rather than lifts,
the R6/R9 block on KG default-enablement.

Given the "Small N" limitation above, this result should not be read as final proof KG can never
help — 16 runs and 3 defective fixtures is not enough to rule that out in general. But it clears
the bar the R6/R9 gap asked this benchmark to resolve: there is now a real, executed measurement,
and it points the same direction as the cost-only audits, not away from them.

### Option A — Remove the KG wiring (recommended)

Rip out the `graph` feature/`[graph]` config path rather than continuing to carry it opt-in and
unresolved. Justification: it is cost-positive (+0.7% to +3.6% input tokens on every fixture,
Table 3) with zero measured value across all 4 fixtures, and in 2 of 16 runs it produced a
confidently wrong "this is fine" claim about genuinely broken code — a worse outcome than doing
nothing. Three consecutive audits (R6, R9, this one) have now failed to find a value case for it,
each with a different angle (cost-only, cold/warm parity, cost+value). Removing it also drops the
disk-cache/schema-versioning/`aptu-coder-core` dependency surface (AGENTS.md: "Structural graph
context... disk-cached by commit SHA... schema-hash header") for a feature that has never been
default-on and has not paid for itself opt-in either.

### Option B — Keep it gated, fix three things first, then re-benchmark

Only worth choosing if there's a specific reason to believe the graph could still pay off with
more investment. If so, do not re-attempt default-enablement until all three land:

1. **Diagnose the fixture 1 (broken caller) 0/4.** It's the one fixture that isolates a genuinely
   cross-file, diff-invisible defect — exactly the case a call graph exists to catch — and both
   configs missed it in all 4 runs. Inspect the actual rendered `graph_context` string sent to the
   model for those calls: if the `http.rs` caller edge isn't in the render at all, that's a
   blast-radius/rendering bug, not evidence graphs can't help cross-file.
2. **Fix the false-reassurance failure mode.** Read the rendered graph text behind the two runs
   that wrongly declared broken code correct (#1565 run2, #1566 run1). If the render is stale,
   ambiguous, or doesn't distinguish "caller already updated" from "caller still references the old
   signature," that's a concrete, fixable bug — and until it's fixed, KG is not just low-value but
   actively unsafe to enable for anyone.
3. **Re-run with a larger N** (more fixtures, ideally non-toy real-world PRs) before revisiting
   default-enablement — 16 runs is enough to flag a problem but not enough to fully absolve or
   condemn the feature once (1) and (2) are addressed.

Absent a concrete reason to invest in (1)-(3), Option A is the lower-maintenance, evidence-backed
choice.

## Reproduction

```bash
# 1. Build release binary from main
cargo install --path crates/aptu-cli --profile release

# 2. Create the 4 fixture branches + draft PRs against clouatre-labs/aptu (never merge):
#    - fixture/broken-caller
#    - fixture/dead-code-path
#    - fixture/wrong-trait-impl
#    - fixture/clean-control
#    Each PR: 1-3 files, 10-30 lines, matching the defect descriptions above.

# 3. No KG config: remove ~/.config/aptu/config.toml.
#    KG config: ~/.config/aptu/config.toml with:
#    [graph]
#    enabled = true
#    max_depth = 4
#    max_nodes = 50000
#    cache_ttl_hours = 24

# 4. Clear graph cache between configs only:
rm -rf ~/.local/share/aptu/graph/clouatre-labs/aptu/

for pr in <broken_caller_pr> <dead_code_pr> <wrong_trait_pr> <clean_control_pr>; do
  for run in 1 2; do
    aptu pr review $pr --repo clouatre-labs/aptu -o json 2>/dev/null \
      | jq '{prompt_chars: .ai_stats.prompt_chars, input_tokens: .ai_stats.input_tokens,
             cost_usd: .ai_stats.cost_usd, duration_ms: .ai_stats.duration_ms,
             verdict: .review.verdict, comments: .review.comments}'
  done
done

# 5. Cleanup: close all 4 draft PRs, remove the config file and the graph cache directory.
```

Do NOT clear cache between runs of the same config (run 1 is cold, run 2 is warm).
