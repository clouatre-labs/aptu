# Audit: Graph-Context Prompt Injection Effectiveness — August 2026

Date: 2026-08-26  
Toolchain: aptu built from source (`target/release/aptu`) — confirmed petgraph compiled in via the `graph` cargo feature; the installed 0.10.16 release binary lacks it.  
Data: 8 merged aptu PRs x 2 conditions (`APTU_GRAPH__ENABLED=false`/`true`), `--deep` held constant, same OpenRouter `mistralai/mistral-small-2603` model on both sides.  
Method: `aptu pr review --dry-run --no-comment --force` — real AI calls, nothing posted.  
Scope: `crates/aptu-core/src/ai/review_context.rs` (graph-context build + budget-drop tiers), `crates/aptu-core/src/graph/{cache,query}.rs`.

---

## Purpose

Determine whether the structural-graph prompt-injection feature (added in #1408) measurably changes PR-review prompt size, token usage, cost, or latency in practice, and why.

---

## Method

8 merged PRs, each run twice via `aptu pr review --dry-run --no-comment --force`: `APTU_GRAPH__ENABLED=false` vs `true`, `--deep` held constant, identical model both sides. Nothing posted; deltas computed off/on within each PR.

---

## Results

*Table 1: Per-PR deltas, graph off vs on.*

| PR# | diff (+/-) | Δprompt_chars_final | graph_chars (on) | graph_context in budget_drops (on) | Δinput_tokens | Δcost_usd | Δduration_ms |
|---|---:|---:|---:|:---:|---:|---:|---:|
| 1473 | 8 | 0 | 0 | no | -12 | -0.000776 | -759 |
| 1504 | 50 | 0 | 0 | no | 0 | +0.000127 | +71 |
| 1476 | 79 | +885 | 855 | no | +242 | +0.000051 | -137 |
| 1511 | 88 | +10,934 | 10,904 | no | +2,945 | -0.000240 | +1,221 |
| 1485 | 125 | 0 | 0 | no | 0 | -0.001139 | +4 |
| 1488 | 272 | 0 | 0 | no | 0 | -0.001213 | +296 |
| 1495 | 332 | 0 | 0 | yes | 0 | -0.003806 | +976 |
| 1484 | 643 | 0 | 0 | no | 0 | -0.001905 | -322 |
| **mean** | — | 1,477 | — | — | 397 | -0.001112 | 169 |
| **max*** | — | 10,934 | — | — | 2,945 | -0.003806 | 1,221 |

*max = largest-magnitude delta in the sample.

### Plain reading of the results

- For 6 of 8 PRs, enabling graph context changed nothing measurable (`Δprompt_chars_final = 0`, `Δinput_tokens = 0`) — the graph section was empty regardless of the flag.
- Where it populated (1476, 1511), the effect on prompt size/tokens was real and scaled with PR complexity — negligible for 1476 (+885 chars, +242 tokens) but material for 1511 (+10,934 chars, +2,945 input tokens, +26% duration).
- `Δcost_usd` is not a reliable signal here: several pairs show identical or near-identical input/output tokens but divergent cost (1485, 1488, 1495, 1484 all show the "on" run costing 25-80% less than "off" despite equal or higher token counts). Something other than the graph toggle drives that variance — token deltas are the trustworthy metric in this dataset, cost deltas are not.
- The existing budget cap was never observed dropping a *populated* graph section in this sample; whether it does under heavier load is unmeasured here.

---

## Findings

### F1: `budget_drops` records empty-section drops as real drops (LOW / BUG)

**Severity:** Low  
**Category:** BUG  
**Tracking:** issue #1515

`apply_budget_drops` (`review_context.rs:390-401`) pushes `"graph_context"` onto `budget_drops` whenever the running prompt estimate is still over `max_prompt_chars` at that tier — unconditionally, regardless of whether `graph_context` had any content to clear.

```rust
// review_context.rs:390-401
if estimated_size > max_prompt_chars {
    tracing::warn!(..., "Dropping section: prompt budget exceeded (graph_context tier)");
    let dropped_chars = graph_context.len();
    graph_context.clear();
    estimated_size -= dropped_chars;
    budget_drops.push("graph_context".to_string());
}
```

**Evidence:** PR 1495 in the sample shows `graph_context in budget_drops: yes` with `graph_chars: 0` on both the off and on runs — this is bookkeeping over an already-empty section, not evidence the cap absorbed a populated graph. No PR in the 8-PR sample shows a *populated* graph section (nonzero `graph_chars`) being dropped by the budget.

**Impact:** `budget_drops` cannot be trusted as a signal for "the cap discarded real graph data" without manually cross-checking `graph_chars` — as currently emitted it's misleading telemetry.

### F2: graph population only fires on new symbol declarations, not modified ones (MEDIUM / BUG)

**Severity:** Medium  
**Category:** BUG  
**Tracking:** issue #1516

`derive_modified_symbols` (`review_context.rs:719-750`), which feeds `build_ctx_graph` (`review_context.rs:637-694`), extracts symbol names via `SYMBOL_RE`, which matches only **added** lines (`+fn`, `+struct`, `+enum`, `+trait`, `+impl`) that introduce a *new* declaration. A diff that only edits the body of an existing symbol — the dominant PR shape, including both of the two largest diffs in this sample (1488 at 272 lines, 1484 at 643 lines) — produces zero matches. With an empty symbol set, `find_modified_nodes` + `blast_radius` (`review_context.rs:656-673`) have nothing to query and return an empty subgraph.

**Evidence:** 6 of 8 PRs show `graph_chars: 0` on the "on" run despite `APTU_GRAPH__ENABLED=true` and `--deep` — including both largest diffs, which are large specifically because they edit a lot of existing code, not because they add new symbols.

**Impact:** the feature is silently inert for the dominant PR shape in practice. This — not the budget cap (F1) — is why the sample shows "no measurable effect" in 75% of cases.

### F3: cost deltas are not a reliable signal in this dataset (INFO)

**Severity:** Info  
**Category:** MEASUREMENT

4 of 8 pairs (1485, 1488, 1495, 1484) show 25-80% cost divergence between off/on despite equal or higher token counts on the "on" side. Cost tracks something other than the graph toggle in this data. `Δinput_tokens` is the trustworthy metric here; `Δcost_usd` should not be used to judge this feature without controlling for whatever else varies cost between runs.

### F4: where it populates, the mechanism works correctly and scales with complexity (INFO)

**Severity:** Info  
**Category:** POSITIVE

PR 1476: +885 chars / +242 tokens (negligible). PR 1511: +10,934 chars / +2,945 input tokens / +26% duration (material). This confirms the graph construction, query, and rendering path (`review_context.rs:637-694`) works correctly when given a non-empty symbol set — the entire gap is upstream, in symbol derivation (F2), not in graph construction or query.

---

## Recommendations

### R1: Extend symbol derivation to cover modified, not just new, symbols (fixes F2)

**Priority:** Medium

Resolve each diff hunk's changed line range to its *enclosing* symbol using the AST-context/ symbol-index data already available via `ast_output`, rather than regexing only newly-added declaration lines. This is the fix that makes the feature fire on the common case (editing existing code), which is most of what `pr review` sees in production.

### R2: Make `budget_drops` reporting content-conditional (fixes F1)

**Priority:** Low

Only push a tier name onto `budget_drops` when `dropped_chars > 0`, so the field is trustworthy telemetry for "the cap actually discarded populated data" without needing to cross-reference `graph_chars` by hand.

### R3: Re-run this benchmark after R1 lands (info)

**Priority:** Info

Re-run on a sample that includes body-only-edit PRs — this sample already does, which is what exposed F2 — to confirm `graph_chars` becomes nonzero for the previously-empty 6, and to get a clean read on the feature's real prompt/cost/latency impact once it actually fires.

---

## Summary

*Table 2: Findings.*

| ID | Severity | Category | Finding | Status |
|---|---|---|---|---|
| F1 | Low | BUG | `budget_drops` records empty-section drops as real drops | OPEN (#1515) |
| F2 | Medium | BUG | Graph population only fires on new symbol declarations, not modified ones | OPEN (#1516) |
| F3 | Info | MEASUREMENT | Cost deltas unreliable in this dataset; use token deltas instead | INFO |
| F4 | Info | POSITIVE | Mechanism works correctly once given a non-empty symbol set | INFO |

*Table 3: Recommendations.*

| ID | Priority | Fixes | Recommendation |
|---|---|---|---|
| R1 | Medium | F2 | Extend symbol derivation to cover modified (not just new) symbols |
| R2 | Low | F1 | Make `budget_drops` reporting content-conditional |
| R3 | Info | — | Re-run benchmark after R1 lands |
