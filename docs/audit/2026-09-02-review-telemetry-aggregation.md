# Audit: Review-Context Telemetry Aggregation — September 2026

Date: 2026-09-02

Scope: Whether aptu's PR review pipeline needs an architecture change to avoid context saturation on large pull requests (prompted by a Claude Code Architect exam scenario on subagent delegation), and — since it does not — what evidence infrastructure would let a future decision on that question be made safely and factually instead of speculatively.

Companion documents: [Audit: KG Market Validation](2026-08-29-kg-market-validation.md); [Audit: KG Benchmark v2 — Cost + Value (ROI)](2026-08-28-kg-benchmark-v2-roi.md)

## Purpose

A Claude Code Architect practice exam question described a CI review agent that reads every changed file plus dependencies into one long-lived session; by synthesis time, findings about early-read files are lost or contradicted. The correct fix is subagent delegation with context isolation: verbose exploration stays in disposable subagent contexts, and only compact structured findings (location, issue, severity) cross back into the coordinating session.

This audit asks whether that failure mode, and that fix, apply to aptu's own PR review pipeline and to `aptu-github-app`. The verdict is **NOT APPLICABLE** to the failure mode as described, and **NOT YET JUSTIFIED** for the fix. aptu's review is a single bounded-context HTTP completion call, not an accumulating agentic session, so the specific saturation mechanism in the exam does not exist here. But aptu does drop content under budget pressure today (truncated files, dropped patches, skipped call-graph context), and there is currently no way to know how often that happens across real PRs. Building a decomposition architecture without that evidence would repeat the pattern already documented in the KG audits: eight dated investigations to validate and then remove a context-enrichment feature that was built ahead of evidence. This audit recommends closing the evidence gap first, using infrastructure that already exists.

## Why the exam's failure mode does not transfer

`review_pr` (`crates/aptu-core/src/ai/provider/review.rs`) issues one system message and one user message, built once from `review_context.rs`, and parses one structured response. There is no multi-turn session accumulating raw file reads over time, so "early findings lost by the time of synthesis" has no mechanism to occur. aptu also does not embed the Claude Agent SDK or any subagent runtime; it calls AI providers directly via an OpenAI-compatible interface (`ai::registry`). The literal fix from the exam — isolated subagent context windows — is not an available primitive inside aptu's own binary.

Where large PRs are handled today is budget enforcement, not decomposition: `max_diff_chars` (200k), `max_patch_chars_per_file` (25k, oversized patches dropped whole), and `min_budget_for_call_graph` (call-graph context skipped below a threshold), defined in `ReviewConfig` and validated at load time by `validate_consistency()`. `aptu-github-app`'s `pr-review.yml` calls this same path unconditionally, once per PR regardless of file count, passing `max-prompt-chars: '200000'` and `max-full-content-files: '0'`.

## What telemetry already exists

This is the material finding of this audit: aptu already instruments the exact signal needed to evaluate the failure mode, and it is wired into the shipped action — it simply is not aggregated anywhere.

- `crates/aptu-core/src/metrics.rs` defines `ReviewContextRecord`: `files_total`, `files_with_patch`, `files_truncated`, `truncated_chars_dropped`, `ast_context_chars`, `call_graph_chars`, `budget_drops` (e.g. `"call_graph"`, `"full_content"`), `prompt_chars_final`, `max_prompt_chars`, `model`, `finish_reasons`, plus `pr` (`owner/repo#number`) and `github_actor`.
- It is fire-and-forget and a no-op unless `APTU_CONTEXT_FILE` is set (`metrics.rs:104-106`) — never fails the caller.
- `action.yml` (the composite action `aptu-github-app`'s `pr-review.yml` invokes at `clouatre-labs/aptu@<tag>`) sets `APTU_CONTEXT_FILE` and `APTU_METRICS_FILE` to per-run `runner.temp` paths (`action.yml:579-580`), renders a human-readable "Context budget" section into `GITHUB_STEP_SUMMARY` — files total/truncated, chars dropped, prompt budget percentage, drop reasons (`action.yml:740-758`) — and uploads both JSONL files as run artifacts with 7-day retention (`action.yml:769-776`).

So every consuming repo's PR review already produces this data, per run, today. The gap is that it lives only as a human-readable summary on one run's page and a 7-day artifact scattered per repo per run. Nothing rolls it up, so the question this audit exists to answer — "does context truncation actually happen often enough on real PRs to justify a decomposition architecture" — cannot currently be answered without manually opening every run's summary across every consuming repo.

## Privacy and security constraints on closing the gap

Aggregating this data safely means the aggregate must never carry identifying fields past the point of aggregation:

- `pr` (`owner/repo#number`) and `github_actor` identify a repository, a PR, and a person. Every other field (`files_total`, `files_truncated`, `truncated_chars_dropped`, `budget_drops`, `prompt_chars_final`, `max_prompt_chars`, `model`, `finish_reasons`) is a count, size, enum, or model identifier — none is code, a file path, or a diff.
- No code, diff content, file name, or file path is captured today; that must remain true — the existing `PromptConfig` byte caps already exist specifically as prompt-injection/content-exposure guards, and this proposal must not create a new path that leaks reviewed source.
- Aggregation must happen *before* anything leaves the ephemeral runner or a repo's own artifact storage: fold each run's JSONL into counters (`reviews_total`, `truncation_events_total`, `files_truncated_total`, `budget_drop_reason_counts`, `model_tier_counts`, `prompt_budget_pct_histogram_bucket`) and discard `pr`/`github_actor` at that point. What is retained centrally is never per-PR, only summed.
- Default OFF, opt-in only, consistent with aptu's existing posture (no `GITHUB_TOKEN` env var, OAuth device-flow credentials in the OS keyring, XDG-local storage, no phone-home by default). A repo operator explicitly enables rollup; nothing changes for anyone who doesn't.
- No new secrets or scopes: any central collection point should accept only the pre-aggregated counters, over TLS, with no repo/PR/user fields in the schema — so there is nothing sensitive to protect even if the endpoint is compromised.

## Recommendation

### Option A — Aggregate what already exists (recommended)

1. Add an opt-in rollup step to `action.yml` (or a follow-up composite action) that reads the existing `APTU_CONTEXT_FILE`/`APTU_METRICS_FILE` JSONL already produced this run, computes the anonymized counters above, and appends only those counters to a durable, operator-controlled location — a dedicated branch/artifact in the operator's own repo for self-hosted use, or (since `aptu-github-app` already runs a Cloudflare Worker as a hosted, org-operated service) an aggregate-only endpoint on that Worker for hosted use. Raw per-PR JSONL is never transmitted off the run that produced it.
2. Pre-register the decision rule before collecting data, following the KG benchmark's own methodology (catch/miss/false-positive framing, explicit thresholds pre-registered before the experiment): e.g., "if truncation events (`files_truncated > 0` or non-empty `budget_drops`) occur in more than X% of reviewed PRs over N weeks, scope a time-boxed map-reduce decomposition experiment; otherwise, the current single-call budget-cap architecture is sufficient and no further work is justified."
3. Run for a fixed observation window, then revisit this audit with the actual rate. If the rate is low, close this line of investigation the same way the KG audits closed the graph question — with evidence, not intuition.

### Option B — Build a new dedicated telemetry pipeline (not recommended)

Standing up a new collection service duplicates `ReviewContextRecord`, which already captures the needed fields, and defers the actual answer behind unnecessary build cost. There is no justification for this scope until Option A's low-cost rollup shows the existing per-run signal is insufficient.

## Summary

The Claude Code Architect exam's context-saturation failure mode does not structurally exist in aptu's single-call, budget-capped review pipeline, and its literal fix (Agent-SDK subagent isolation) has no analog inside aptu's own runtime. What aptu does have is undermeasured: a fully-instrumented per-run signal (`ReviewContextRecord`, already wired into the shipped `action.yml`, already rendered into job summaries and uploaded as artifacts) that nobody aggregates across runs. Building a map-reduce or subagent-style decomposition now would repeat the KG pattern of shipping context-architecture complexity ahead of evidence. The recommended next step is a small, opt-in, anonymized rollup of data aptu already collects, with a pre-registered threshold for when the harder architecture work would actually be justified.

## Sources

- https://arxiv.org/abs/2307.03172 (Liu et al., "Lost in the Middle: How Language Models Use Long Contexts")
- https://code.claude.com/docs/en/agent-sdk/subagents
