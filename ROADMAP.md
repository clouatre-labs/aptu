# Aptu Roadmap

Strategic focus: **GitHub Actions and GitHub App** for AI-powered PR review and issue triage. Simple UX, smart defaults, bring-your-own-key. OSS-first, low maintenance, performance-conscious.

Validated against source code (`aptu-coder`), GitHub documentation, and competitive analysis (May 2026).

---

## Design Principles

- **Simple by default, configurable by exception.** Smart defaults that work without any config file.
- **The cheapest AI call is the one you skip.** Gate before calling; trim before sending.
- **Metrics are first-class.** Every run emits structured JSONL. You cannot optimize what you cannot measure.
- **Standard file formats.** AGENTS.md, SARIF, JSONL -- not invented-here schemas.
- **Low maintenance surface.** Fewer crates, fewer features, less to break.

---

## Active Work (ordered by priority)

### P0 -- Review Quality

These fix silent bugs that produce wrong or irrelevant reviews today.

**Fix PR file pagination -- PRs with >30 changed files silently drop remaining files** (#1222)
`list_files()` uses octocrab's default page size (30). Confirmed: `pulls.rs:119-124`, no `per_page` or pagination loop. Fix: collect all pages up to GitHub's 300-file cap.

**Detect and recover from GitHub-truncated patches** (#1223)
GitHub silently truncates patches >1 MB or >10,000 lines; the `patch` field ends mid-hunk. Confirmed: `pulls.rs` has no hunk integrity check. Fix: detect mid-hunk truncation (patch ends without closing context line); fall back to full-file fetch via Contents API at PR head SHA.

**Add explicit model guidance when content is truncated at budget boundary** (#1224)
The prompt appends `[Body truncated - original length: N chars]` passively. The model invents context for what it cannot see. Confirmed: `provider.rs:610, 1012, 1047`. Fix: prepend explicit instruction: "Content was truncated at the budget boundary. Review only what is shown. Do not infer or speculate about content beyond the cut."

### P1 -- Token Observability and Efficiency

**JSONL token-usage artifact and GITHUB_STEP_SUMMARY cost table** (#1225)
`AiStats` already carries all needed fields (`history.rs:20-40`). Nothing is exported. Pattern adopted from `aptu-coder`'s channel-based observability: fire metric events at return time, background-append to JSONL, zero blocking on the hot path.

Implementation: env var `APTU_METRICS_FILE` triggers append (no new CLI flag). `action.yml` sets the env var, then adds a shell step to write a markdown table to `GITHUB_STEP_SUMMARY` and upload via `actions/upload-artifact@v4`. Outputs: `input-tokens`, `output-tokens`, `cost-usd`, `duration-ms`.

**Add cache_read_tokens and cache_write_tokens to UsageInfo** (#1226)
Required for the Effective Tokens metric `ET = m * (1.0*I + 0.1*C + 4.0*O)`. Confirmed missing: `types.rs:81-94`. Both Gemini and Anthropic report cache tokens in their API responses.

**Relevance gate -- skip AI review for docs-only and dependency-bump PRs** (#1227)
Review runs unconditionally on every `pull_request` event. Confirmed: `action.yml`, no conditional logic before the AI call. Fix: check changed file extensions before invoking the model. All `*.md`, `*.txt`, `*.lock`, `*.toml` with version-only diff = emit a lightweight summary at zero AI cost.

Pattern adopted from `aptu-coder`'s test-file filtering (`test_detection.rs`): path heuristics, no parsing required, output-layer decision.

**Read AGENTS.md and .github/instructions/pr-review.md as review context** (#1228)
Two files only. No bloat.

Priority order:
1. `AGENTS.md` (root) -- vendor-neutral cross-tool standard (60,000+ repos, August 2025)
2. `.github/instructions/pr-review.md` -- aptu-specific override

Optional config: `[review] instructions_file = "path/to/file"` and `action.yml` input `instructions-file` override both defaults with a single custom path.

`.github/copilot-instructions.md` is deliberately excluded -- it is Copilot's config, not ours. Maintainers who want aptu-specific guidance use `AGENTS.md` (already there) or `.github/instructions/pr-review.md`.

Implementation: one Contents API call with PR head SHA, no checkout. Cap at 4,000 chars (matches Copilot's enforced limit). Fail silently if neither file exists. Strip YAML frontmatter before injecting.

**Remove aptu-mcp crate** (#1229)
The MCP server is a thin adapter (2,376 lines, 4 files) with zero business logic beyond parameter marshaling. Confirmed: `server.rs` delegates every tool call directly to `aptu_core::facade::*`. Removing it eliminates `rmcp` dependency upgrades, separate Homebrew formula, and binary build targets. Not a one-way door -- the facade API remains clean.

### P2 -- Cost Reduction

**Prompt caching for system prompt and repo context** (#1230)
Depends on #1226 (cache token fields). System prompt (5,000 chars) + AST/call-graph context do not change between runs on the same repo. Cache-read cost is 0.1x input cost on both Gemini and Anthropic. Expected 10-30% cost reduction on active repos.

### P94 -- GitHub App

One-click installation, fork PR support without `pull_request_target` complexity, foundation for future monetization. Uses the same `aptu-core` facade. New delivery layer, not a rewrite.

Tracked in #94.

---

## Effective Tokens Metric

Normalized throughput signal that is comparable across operations and over time without a per-model pricing table.

```
ETU = 1.0 * I + 0.1 * C + 1.25 * W + 5.0 * O
```

Where I = input tokens, C = cache read tokens, W = cache write tokens, O = output tokens.

Weights are the structural Anthropic cache pricing ratios (confirmed May 2026, stable across all model generations since Claude 3):
- Cache read: 0.1× base input (90% discount)
- Cache write: 1.25× base input (5-min TTL, conservative; 1-hr TTL is 2×)
- Output: 5× base input (consistent across Haiku 4.5, Sonnet 4.6, Opus 4.7)

The per-model multiplier `m` from the earlier formulation was removed: it required a drift-prone per-model pricing table. ETU is unit-less (input-equivalent tokens), not dollars. A 10% ETU reduction is a genuine 10% cost reduction regardless of which model is in use.

Track ETU per run in the JSONL artifact and in `GITHUB_STEP_SUMMARY`. Emitted as `effective_token_units` in `AiStats` and in `action.yml` outputs.

---

## Patterns Adopted from aptu-coder

`~/git/clouatre-labs/aptu-coder` was audited for transferable patterns (May 2026). Selected adoptions:

- **Channel-based JSONL observability** (`metrics.rs`): fire metric events into unbounded channel at return; background writer appends to JSONL. Zero blocking on hot path. Applied to #1225.
- **Path-heuristic relevance filtering** (`test_detection.rs`): skip or deprioritize files by path pattern without parsing. Applied to #1227 (docs-only gate) and future test-file deprioritization in review.
- **Output-size enforcement** (`output_size` test, `SIZE_LIMIT` constant): enforce token budget at test time, not only at runtime. Worth adopting in `provider.rs` tests.
- **Graceful degradation via `lock_or_recover`** (`cache.rs`): on poisoned mutex, clear and continue rather than panic. Applicable to aptu's disk cache layer.

Patterns audited and not adopted:
- Summary-first cursor-paginated output: aptu is a CLI/Action, not an MCP server; streaming pagination does not apply to single-run AI calls.
- Per-language AST extractors: aptu already has its own AST context pipeline in `provider.rs`.

---

## Removed from Roadmap

- **iOS App** (Phase 2 in SPEC.md): not aligned with GitHub Actions / App focus.
- **Gamification / Leaderboards** (Phase 3 in SPEC.md): deferred; requires platform and user base first.
- **MCP Server** (`aptu-mcp`): removed (see #1229).

These remain in SPEC.md for historical context.

---

## Issue Index

| # | Title | P |
|---|---|---|
| #1222 | Fix PR file pagination (>30 files silently dropped) | P0 |
| #1223 | Detect and recover from GitHub-truncated patches | P0 |
| #1224 | Add explicit model guidance on truncated content | P0 |
| #1225 | JSONL token-usage artifact + GITHUB_STEP_SUMMARY | P1 |
| #1226 | Add cache_read_tokens / cache_write_tokens to UsageInfo | P1 |
| #1227 | Relevance gate for docs-only / dep-bump PRs | P1 |
| #1228 | Read AGENTS.md and .github/instructions/pr-review.md | P1 |
| #1229 | Remove aptu-mcp crate | P1 |
| #1230 | Prompt caching (Gemini / Anthropic) | P2 |
| #94 | GitHub App | P94 |
