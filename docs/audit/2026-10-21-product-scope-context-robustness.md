# Audit: Product Scope and Context-Window Robustness — October 2026

Date: 2026-10-21  
Data: `~/.pi/agent/sessions` (38 sessions, 907MB), `~/.local/share/aptu/`, GitHub search API (commenter:aptu-dev[bot], org:clouatre-labs), `.github/workflows/`, `crates/aptu-core/src/ai/`, `crates/aptu-core/src/config/`, `crates/aptu-cli/src/cli.rs`, aptu-github-app worker and reusable workflows, competitor/tooling research (CodeRabbit, Greptile, Graphite, Copilot code review, Qodo, pr-agent)  
Issues: #1754, #1755, #1756, #1757, #1758, #1759, #1760, #1761, #1762, #1763  
Method: four parallel read-only scouts (feature/prompt inventory, failure-mode data-flow trace, competitor research, real-world usage analysis), followed by direct verification of every load-bearing claim against source and live GitHub state before issue creation

## Purpose

Assess whether aptu has accreted features and prompt weight beyond its two dominant use cases (`aptu pr review` via CLI and aptu-github-app), identify the root cause of reported context-window failures, and map 2026 state-of-the-art practices onto a targeted optimization plan.

## Usage reality

*Table 1: Observed aptu-dev[bot] activity across clouatre-labs (GitHub search API, Oct 2026).*

| Operation | Bot comments | Share |
|---|---|---|
| PR review | 1,756 | ~97% |
| Issue triage | 64 | ~3% |

Growth is steep: 86 comments in Apr–Jun 2026, 1,211 in Jul–Sep, 522 in the first three weeks of Oct. Ten clouatre-labs repos carry `.github/aptu.yml`; feature adoption within them: triage 8/8, review 8/8, scan 7/8, lint 1/8. All AI-enabled repos use `openrouter` + `google/gemma-4-26b-a4b-it`. The local CLI's contribution-history feature has never recorded an event (`~/.local/share/aptu/history.json` does not exist).

Clarification on dogfooding: the aptu repo reviews itself through its own `.github/workflows/pr-review.yml` (gemini-3.1-flash-lite, openrouter/mercury-2 fallback), not through the GitHub app; the app serves other repositories. This is by design and is not a gap.

## Findings

### C1 — Context-window failures trace to unenforced model limits

**Severity:** High  
**Area:** Review robustness  
**Finding:** The registry parses `context_window` (`ai/registry/parsing.rs:82`) but it is enforced nowhere (zero non-test references). Budgeting is char-only arithmetic with three defects: `max_diff_chars` (200,000) exceeds `max_prompt_chars` (120,000) (`config/review.rs`); `estimate_pr_size` (`ai/review_context/budget.rs:284`) omits the system prompt, custom guidance, and the repository instructions file (AGENTS.md / instructions-file, fetched by `github/instructions.rs`); and a provider 400/413 context-length error is non-retryable (`ai/provider/http.rs:44-83`, `retry.rs:76-106`), so the review hard-fails. The app workflow compounds this with `max-prompt-chars: '200000'` against gemma-class windows and `continue-on-error: true`, which converts failures into silent skips.  
**Evidence:** Direct source reads; aptu-github-app `.github/workflows/pr-review.yml:70,85`.  
**Issues:** #1754, #1755, #1756

### C2 — Prompt files carry cuttable redundancy, with a modest ceiling

**Severity:** Medium  
**Area:** Prompt efficiency  
**Finding:** All three guidelines files end with "respond ONLY with valid JSON" boilerplate (redundant with structured output); `pr_review_guidelines.md` re-describes the JSON schema in prose alongside `pr_review_schema.json` (drift-prone dual specification); the truncation section (~700 chars) and dependency-release-notes section (~500 chars) over-explain. Roughly 60% of guideline prose (~3-4k chars/call) is cuttable. This is a cost and maintenance win, not the cause of the context failures.  
**Issues:** #1757

### C3 — Several features are cut candidates on usage evidence

**Severity:** Medium  
**Area:** Product scope  
**Finding:** PR review is ~97% of activity; the local CLI records zero history events. Cut candidates: `issue lint` (1/10 repos), `pr queue`, `completion`, contribution history, dep-release-note enrichment, and `ai/judge.rs` (optional TypeSafe integration). Keep: review, triage, scan, `aptu.yml` gating, instructions-file ingestion (already implemented in `github/instructions.rs`, covering the AGENTS.md convention pattern the leading tools adopted).  
**Issues:** #1758, #1759

### C4 — Pre-computed AST injection is losing to just-in-time retrieval

**Severity:** Medium  
**Area:** Review quality  
**Finding:** Real logs show repeated `Dropping section: prompt budget exceeded, section="ast_context"` warnings. The 2026 tooling landscape (CodeRabbit's context engine, Copilot's agentic exploration, Qodo's multi-agent review) abandoned fixed AST dump injection for diff-scoped retrieval and on-demand reads. Near-term: make AST/call-graph injection opt-in; long-term: design diff-scoped retrieval before removing the 9-language pipeline.  
**Issues:** #1760

### C5 — State-of-the-art quality practices worth adopting

**Severity:** Medium  
**Area:** Review quality  
**Finding:** Two practices dominate the field's noise reduction: a cheap verification pass that confirms each finding against the code before posting, and confidence/severity gating with style suppression. A third, incremental review on push (diff-since-last-reviewed-commit), cuts repeat-push cost sharply. Convention-file ingestion is already done (see C3). Sources: Martian 2026 benchmark, Macroscope 118-bug benchmark, GitHub Copilot code review docs, Qodo 2.0 announcement.  
**Issues:** #1761, #1762

### C6 — Install base outside clouatre-labs is unmeasurable

**Severity:** Low  
**Area:** Observability  
**Finding:** Worker telemetry aggregates only global review-context counters into one KV rollup; there is no per-installation tracking, so the product-scope decisions above cannot be revisited with data once the app grows beyond the 10 known repos.  
**Issues:** #1763

## Recommendations

*Table 2: Recommended sequence.*

| Order | Theme | Issues | Rationale |
|---|---|---|---|
| 1 | Make reviews never fail on context | #1754, #1755, #1756 | Small diffs, removes hard failures and silent skips |
| 2 | Trim scope to the product | #1758, #1759, #1757 | Usage-justified cuts and prompt slimming |
| 3 | Modernize review quality | #1760, #1761, #1762 | Opt-in AST, verification pass, incremental review |
| 4 | Instrument the install base | #1763 | Enables future evidence-based scope decisions |

## Verification notes

Every claim in this report was re-checked against source or live GitHub state after the scout phase; three scout claims were corrected before issue creation: the dogfooding gap (aptu uses its own Actions workflows by design), the error variant name (`EmptyReviewContext`, not `NoSurvivingPatches`), and AGENTS.md ingestion listed as missing when `github/instructions.rs` already implements it. The bot-activity counts (1,756 PR / 64 issue comments) were confirmed via the GitHub search API.
