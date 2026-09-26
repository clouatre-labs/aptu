# Audit: Jev Typed-Judgment Opportunities in aptu — September 2026

Date: 2026-09-26
Data: `crates/aptu-core/src/config/ai.rs`, `crates/aptu-core/src/security/validator.rs`, `crates/aptu-core/src/bulk.rs`, `crates/aptu-core/src/ai/types.rs`, `crates/aptu-core/src/cache.rs`, `crates/aptu-core/src/history.rs`, `crates/aptu-core/src/ai/registry/`, `crates/aptu-core/Cargo.toml`, external repos [Tech-Byte-Frontier/jevgate](https://github.com/Tech-Byte-Frontier/jevgate) and [Protocol-Lattice/harness-router](https://github.com/Protocol-Lattice/harness-router), `clouatre-labs/decisions-judge-mcp`, dotfiles audits `2026-09-26-coder-issue-premise-audit.md` / `2026-09-26-premise-gate-validation.md`
Issues: filed with this audit
Method: four parallel read-only scouts (external repos, judge API contract, aptu AI stack, prior gate experiments) followed by three adversarial validation passes with file:line evidence; zero source edits during research

## Purpose

Assess whether the TypeSafe Jev typed-judgment model (a small model answering noul yes/no probabilities, choices, and ordered scores in a single HTTPS request) could simplify aptu or improve its performance — as a model-tier router, as a replacement for LLM completions on structured decisions, or as a pre-filter in bulk triage. Evaluate against KISS with adversarial validation of every claim.

## Summary

*Table 1: Findings and recommendation mapping.*

| ID | Verdict | Area | Finding |
|---|---|---|---|
| J1 | Confirmed | Routing | Do not use Jev as a model-tier router |
| J2 | Corrected | Security | `validate_findings_batch` is dead code; delete it, do not wire Jev into `scan-security` |
| J3 | Conditional | Triage | Bulk triage has no LLM pre-filter; a Jev pre-filter is valuable only on spam-heavy batches |
| J4 | Confirmed | Integration | If adopted, Jev enters as a standalone reqwest module, not an `ai::registry` provider |

No production latency or cost data exists on this machine (`~/.local/share/aptu/history.json` absent; no logs under `~/.cache/aptu` or `~/.local/state/aptu`). Every performance claim below is therefore architectural, not measured.

## Findings

### J1 — Do not use Jev as a model-tier router

**Severity:** Decision (no change)
**Finding:** Model-tier routing is a pure char-count match: `AiConfig::resolve_for_task` (`crates/aptu-core/src/config/ai.rs:188`, thresholds at :214-216 — `Review => 60_000`, `Triage | Create => 8_192`) compares `Option<usize>` against an integer. It performs no I/O. A Jev decision adds network latency (harness-router measured mean 454 ms, median 386 ms per Jev call) plus a fallback path for zero routing gain.

**External evidence:** harness-router's own benchmark (`REPORT.md`/`BENCHMARK.md`) reports its Jev routing made the overall system worse: main-model tokens +105.9%, wall time 115.8 s → 228.9 s, 54 of 58 Jev decisions falling back to the planner. The lesson is that a cheap selector only pays off when it *replaces* an existing call, not when it augments one.

**Caveat (validated):** There is no telemetry today recording which model served a task against outcome quality — the only routing-path log is a `tracing::warn!` for misconfigured routing fields (`config/ai.rs:228-233`). Any future "revisit if small-model quality failures appear" trigger is currently vacuous; establishing that signal would be prerequisite to ever revisiting J1.

### J2 — `validate_findings_batch` is dead code; `scan-security` must stay AI-free

**Severity:** Medium (cleanup) / High (contract guard)
**Finding:** `validate_findings_batch` (`crates/aptu-core/src/security/validator.rs:60`), `build_system_prompt` (:127), `build_batch_validation_prompt` (:157), `send_and_parse` (:196), and `fallback_validation` (:223) form an AI-validation path with **zero callers**. Grep across `crates/aptu-cli/src` and `crates/aptu-core/src/lib.rs` finds no reference; `scan-security` (`crates/aptu-cli/src/commands/scan_security.rs`) performs local pattern matching only.

**Contract:** The no-AI guarantee is documented three times: `docs/CONFIGURATION.md:311` ("aptu scan-security performs local pattern matching and does not invoke AI"), `README.md:34`, `README.md:111`. Wiring any AI call — Jev or otherwise — into this path would break a documented user-facing contract.

**Correction of initial hypothesis:** The initial scout proposed replacing the fallback heuristic with a batched Jev call. Validation refuted this: the path never runs in production, and the ~200 LOC deletion estimate was inflated — the three batch-path functions span ~96 LOC (`validator.rs:127-222`). The correct action is deletion (or explicit wiring with a contract change), not an AI call. Unit tests at `validator.rs:282-369` cover `fallback_validation` and the prompt builders and would be removed with them.

### J3 — Bulk triage has no LLM pre-filter; a Jev pre-filter is conditional

**Severity:** Medium
**Finding:** `process_bulk` (`crates/aptu-core/src/bulk.rs:114-192`, `buffer_unordered(5)` at :183) sends every issue through the full completion path (`analyze_issue`, `crates/aptu-core/src/ai/provider/triage.rs:35`). No early exits exist for locked issues, bot authors, or spam in the triage path. `issue_lint` is deterministic and AI-free but is a separate CLI command (`crates/aptu-cli/src/commands/issue_lint.rs:9`), not composed into triage.

**Typed-judgment shape:** The triage output already contains fields that are structurally Jev questions: `ContributorGuidance::beginner_friendly: bool` (`ai/types.rs:158`, struct :156-162), `ComplexityLevel` 3-way choice (:177-185), `estimated_loc: Option<u32>` (:193). One Jev request can carry all of them batched over one state.

**Conditional value (validated caveat):** A Jev pre-filter adds one network call per issue. On spam-heavy batches, skipped completions dominate and total cost falls; on all-legitimate batches, API call volume doubles under the existing `buffer_unordered(5)` cap. Existing deterministic gates inside `analyze_issue` (secret redaction at `facade/issues.rs:95-104`, prompt-injection blocking at :163) reject but do not triage-skip. Value must be validated per the methodology in the dotfiles premise-gate experiment (three-arm retrospective design; Phase-2 convergence gate "precision trending ≥ 2/3").

**WASM note (corrected):** No new `#[cfg]` gating is required. Existing AI HTTP calls compile under wasm32; only OS-dependent facades use `wasm_unsupported!` (`facade/mod.rs:15`). A Jev HTTP module matches the existing pattern.

### J4 — Integration shape: standalone module, not a registry provider

**Severity:** Decision
**Finding:** The judge contract, verified against the `@typesafe-ai/sdk` source and `decisions-judge-mcp` providers: `POST https://api.typesafe.ai/v1/systemone`, `Authorization: Bearer <key>`; request body `{state, questions, model?}` where `model` is **optional** (SDK default `jev-latest`); all questions in one request, answers in one response; any failure returns a `{fallback: true, error}` envelope; payload capped at 256 KiB. `reqwest` is already a workspace dependency (`crates/aptu-core/Cargo.toml:28`).

`ai::registry` is a static registry of four OpenAI-compatible completion providers (`ai/registry/mod.rs:33-34`); Jev is a typed-question endpoint, not a completion provider — forcing it into the registry would be anti-KISS.

**Patterns that would be new, not follow-ons (validated corrections):**
- Telemetry: `history.rs` writes `history.json` as a JSON array, not JSONL. A per-call JSONL telemetry line is a new pattern.
- Caching: `cache.rs` provides TTL/etag file caching (`CacheEntry<T>` :43-84, `FileCacheImpl` :154-201), not content-hash keying. Hash-keyed response caching (as jevgate does) would be new.

**Guardrails transferred from the coder-skill audits (dotfiles #946/#949):**
- Propose deterministically; Jev validates (confirm-ride). The judge may confirm or soften but never originate a stop; stops require evidence-backed deterministic signals.
- Gate on predicate-positive noul with a validated threshold; re-running a fixed case battery is required after any question re-wording.
- A never-fatal fallback must emit a visible log line — the prior live pilot found a fallback silently never executing, indistinguishable from a skip.
- Methodology ground truth is mechanical: F1 (premise/body edits via GitHub GraphQL `userContentEdits`), F2 (revert commits), F3 (unmerged with superseding fix) — not "closed-as-spam/duplicates."

## Recommendations

1. **No Jev routing.** Keep char-count tier routing. Prerequisite to any revisit: record which model served a task alongside outcome signals.
2. **Delete the dead AI-validation path in `security/validator.rs`** (~96 LOC plus its unit tests) or explicitly decide to wire it — silent dead code on a documented no-AI surface is the worst of both. `scan-security`'s no-AI contract is untouched either way.
3. **Treat the triage pre-filter as an experiment, not a feature.** Behind an opt-in `[judge]` config section, gated on the three-arm retrospective validation with the "precision trending ≥ 2/3" convergence rule before it ever runs by default.
4. **If Jev is adopted:** standalone `typesafe_judge` module in `aptu-core` on reqwest; opt-in config; visible fallback logging; telemetry line per call. Do not extend `ai::registry`.

## References

- https://github.com/Tech-Byte-Frontier/jevgate — Rust CI gate; per-unit evidence + code-composed verdicts; content-hash caching; example run 42 files / 118 requests / ~$0.011
- https://github.com/Protocol-Lattice/harness-router — negative-result Jev router benchmark (tokens +105.9%, 54/58 fallbacks)
- clouatre-labs/decisions-judge-mcp — judge MCP server; REST contract verified against `@typesafe-ai/sdk` `dist/index.mjs:513,548-554,581`
- dotfiles `docs/audit/2026-09-26-coder-issue-premise-audit.md`, `docs/experiments/2026-09-26-premise-gate-validation.md` — gate polarity rules, fallback-visibility gap, three-arm validation design
