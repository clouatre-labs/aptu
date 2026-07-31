# Audit: Prompt Efficiency, Knowledge Graph, and DSPy — July 2026

Date: 2026-07-31
Data: `crates/aptu-core/src/ai/prompts/` (9 files, 10,390 bytes), `crates/aptu-core/src/ai/registry.rs`, `crates/aptu-core/src/ai/provider/review.rs`, `docs/spec/autonomous-coder.md`, cited external research (arXiv, deepwiki, tosea.ai)
Method: Direct code reads of prompt files and provider/registry source; comparison against `autonomous-coder.md` spec sections 11 and 13a; external literature review of DSPy and code-knowledge-graph research

## Purpose

Assess whether aptu's hand-written prompt set carries avoidable token cost, whether the AI provider layer routes calls to model tiers by workload size, and whether an in-process structural graph (as opposed to raw GitHub Contents API strings) would reduce PR-review token spend. Evaluate DSPy and Neo4j as candidate tools against these findings.

## Summary

*Table 1: Findings and recommended issue mapping.*

| ID | Severity | Area | Finding |
|---|---|---|---|
| P1 | High | Prompt efficiency | Schema prose descriptions duplicate guidelines content |
| P2 | High | Prompt efficiency | Full JSON examples embedded in system prompt on every call |
| P3 | High | Architecture | Model-tier routing not implemented despite existing size estimator |
| P4 | Medium | Architecture | In-process structural graph (petgraph + tree-sitter) not implemented |

## Findings

### P1 — Schema prose descriptions duplicate guidelines
**Severity:** High
**Area:** Prompt efficiency
**Finding:** `triage_schema.json` (1,008 bytes) and `pr_review_schema.json` (497 bytes) carry verbose prose field descriptions (e.g. "A 2-3 sentence summary of...") that restate content already present in the corresponding guidelines files (`triage_guidelines.md`, `pr_review_guidelines.md`). The schema is injected in the user turn on every call; the duplicated description text adds tokens with no new information for the model.
**Evidence:** Direct read of `triage_schema.json` and `pr_review_schema.json` against `triage_guidelines.md` (2,233 bytes, 26 lines) and `pr_review_guidelines.md` (3,364 bytes, 43 lines). Estimated ~200 tokens wasted per call.
**Recommendation:** Minify schema field descriptions to type/enum constraints only; keep prose explanation exclusively in the guidelines files.
**Acceptance criteria:** Schema JSON files contain no prose duplicated verbatim or near-verbatim from guidelines files. `tests/prompt_lint.rs` passes. Token count of assembled prompt drops measurably for a representative triage and PR review call.

---

### P2 — Full JSON examples embedded in system prompt on every call
**Severity:** High
**Area:** Prompt efficiency
**Finding:** `triage_guidelines.md` and `pr_review_guidelines.md` embed two full JSON examples (a happy-path case and an edge case) directly in the system prompt. These are sent unconditionally on every triage and PR review call, regardless of issue or PR complexity.
**Evidence:** Direct read of `triage_guidelines.md` (2,233 bytes) and `pr_review_guidelines.md` (3,364 bytes), each containing two embedded JSON example blocks. Total triage system prompt is ~4,200 bytes (~1,050 tokens) before user content is added. Estimated ~150-300 tokens wasted per call.
**Recommendation:** Move the two JSON examples out of the system prompt. Either place them in the user turn (loaded once per session rather than reconstructed) or gate inclusion behind a flag so they are sent only when the schema alone has proven insufficient in testing.
**Acceptance criteria:** System prompt for triage and PR review no longer contains inline JSON examples by default. `tests/prompt_lint.rs` updated and passing. System prompt byte count for both builders is reduced and documented in the PR description.

---

### P3 — Model-tier routing not implemented
**Severity:** High
**Area:** Architecture
**Finding:** `registry.rs` hardcodes a single model per provider; there is no routing logic that selects a cheaper or smaller model for low-complexity workloads. `estimate_pr_size()` exists in `provider/review.rs` but is used only to enforce prompt budget limits (truncation), not to select a model tier. This is Phase 1 of the `autonomous-coder.md` spec and is currently 0% implemented.
**Evidence:** Grep of `registry.rs` confirms one hardcoded model per provider. Read of `provider/review.rs` confirms `estimate_pr_size()` call sites are limited to budget enforcement.
**Recommendation:** Implement model-tier routing keyed on `estimate_pr_size()` (or an equivalent issue-size estimator for triage): route small/simple workloads to a lower-cost model tier (e.g. haiku-class) and larger/complex workloads to a higher-capability tier (e.g. sonnet-class). This affects the majority of calls and has direct, measurable cost impact independent of prompt-text changes in P1/P2.
**Acceptance criteria:** `registry.rs` or a new routing module selects model by workload size for at least PR review; behavior is covered by a unit test asserting tier selection for a small and a large `estimate_pr_size()` input. `docs/spec/autonomous-coder.md` Phase 1 status updated to reflect implementation.

---

### P4 — In-process structural graph not implemented
**Severity:** Medium
**Area:** Architecture
**Finding:** PR review context (AST and call-graph information) is currently assembled from raw strings returned by the GitHub Contents API rather than from a queryable in-process graph. External research on code-knowledge-graph approaches shows substantial token and tool-call reductions, and accuracy gains, for structurally similar workloads (issue resolution, code review).
**Evidence:**
- RepoGraph (ICLR 2025, arXiv:2410.14684): +32.8% relative resolve rate on SWE-bench Lite.
- KGCompass (arXiv:2603.27277): 58.3% resolve rate, 10x fewer tokens, 2.1x fewer tool calls.
- Code-review-graph (deepwiki): median 82x per-question token reduction.
- CodeGraph (tosea.ai): ~35% API cost reduction, ~70% fewer tool calls.
- Neo4j was evaluated and rejected as the backing store (see Non-Findings). `petgraph` (pure Rust, `no_std`-compatible, WASM-compatible) combined with `tree-sitter` is identified as the viable path, consistent with aptu-core's existing `wasm32-unknown-unknown` compilation target and single-binary CLI constraint.
- `autonomous-coder.md` section 11 quote: "We have a knowledge graph. We don't have to maintain files. We don't need an agent.md in your code. We have it in the graph database." — Siddhant Pardeshi, Blitzy.
**Recommendation:** Design and prototype an in-process structural graph built with `petgraph` and `tree-sitter`, replacing or supplementing the current GitHub Contents API string assembly for PR review context. Scope as a SPEC document before implementation given the architectural surface area (review context budgets in `ReviewConfig`, multi-language AST support already present).
**Acceptance criteria:** A SPEC document exists describing the `petgraph` + `tree-sitter` integration, including data model, WASM compatibility plan, and interaction with existing `ReviewConfig` budgets (`max_prompt_chars`, `max_full_content_files`, `max_chars_per_file`, `max_diff_chars`, `max_patch_chars_per_file`). No code changes required to close this finding; a follow-on implementation issue is opened referencing the SPEC.

---

## Non-Findings

- **`tooling_context.md` scoping is correct.** Confirmed by direct read: this file (957 bytes, ~240 tokens) is injected only in `build_pr_review_system_prompt` and does not appear in the triage, label, or create system prompt builders. Not a waste finding.
- **DSPy investigated, not recommended.** DSPy is a Python-only framework with no viable Rust or WASM integration path, incompatible with aptu-core's `wasm32-unknown-unknown` target and Rust-native provider layer. Its GEPA optimizer shows a 9.2x shorter prompt versus MIPROv2, a comparison against another DSPy optimizer, not against a well-tuned hand-written baseline; no token-reduction evidence exists versus aptu's current hand-written prompts. An offline compile workflow (DSPy as an out-of-band R&D tool, not runtime-integrated) is technically feasible but disproportionate to aptu's current prompt set size (9 files, 10,390 bytes total). Verdict: skip.
- **Neo4j investigated, not viable.** Requires a JVM runtime and a server process, incompatible with aptu's single-binary Rust CLI distribution model and the `wasm32-unknown-unknown` compilation target used by `aptu-core`. `petgraph` is the correct in-process alternative (see P4).
- **System prompt preamble duplication (role sentence repeated across the four builder functions in `mod.rs` with only a noun changed) was noted but not written up as a numbered finding.** Estimated savings are trivial relative to P1/P2 and do not warrant a separate issue at this time.
- **Spec alignment beyond Phase 1 is not a current gap.** Ten items from `autonomous-coder.md` were audited; only model routing (P3, Phase 1) is missing today. Remaining items are Phase 2+ design targets, not present shortfalls.

## Recommended issue grouping

| Issue | Findings | Scope |
|---|---|---|
| 1 | P3 | `feat(ai)`: implement model-tier routing (haiku/sonnet) via `estimate_pr_size()` |
| 2 | P1, P2 | `feat(prompts)`: minify schemas and move examples to user turn |
| 3 | P4 | `feat(review)`: in-process structural graph context via `petgraph` + `tree-sitter` |
| 4 | P1-P4 | `docs`: add petgraph integration SPEC document |
