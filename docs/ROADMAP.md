# Roadmap

_Near-Term (next 3-6 months) | Medium-Term (6-18 months) | Long-Term (18+ months)_

This document describes the project direction across three time horizons. Items are based on open issues, the project specification, and known user needs. Dates are approximate and depend on maintainer availability.

## Design Principles

- **Simple by default, configurable by exception.** Smart defaults that work without any config file.
- **The cheapest AI call is the one you skip.** Gate before calling; trim before sending.
- **Metrics are first-class.** Every run emits structured JSONL. You cannot optimize what you cannot measure.
- **Standard file formats.** AGENTS.md, SARIF, JSONL -- not invented-here schemas.
- **Low maintenance surface.** Fewer crates, fewer features, less to break.

## Recently Shipped

- **GitHub App** (#94): `aptu-dev` GitHub App with config-as-code opt-in, mention commands, automatic security scanning, per-installation quotas, and caller-supplied AI key model. Installed from [github.com/apps/aptu-dev](https://github.com/apps/aptu-dev).
- **Structural graph context for `pr review`** (#1420): petgraph BFS blast-radius from changed files, opt-in via `graph` Cargo feature, disk-cached by commit SHA
- **Model-tier routing** (#1416): routes large PRs to a higher-capability model tier automatically based on estimated prompt size
- **Prompt optimisation** (#1415): minified schemas, examples moved to user turn (~2.6k chars saved per call)
- **PR creation automation** (#1130): `aptu pr create --diff <file>` applies a unified diff to a new branch, commits with optional DCO sign-off, and opens a pull request. Includes a security validation pipeline (size cap, path-traversal rejection, `SecurityScanner::scan_diff()` gate) and collision-resistant branch naming.
- **File-based TTL cache eviction** (#1172): `[cache]` config now supports per-field TTL settings (`issue_ttl_minutes`, `repo_ttl_hours`, `file_eviction_days`); stale cache entries are automatically pruned on startup.

## Near-Term (next 3-6 months)

These items address known gaps and complete features already partially implemented.

- **Bulk triage improvements**: better progress reporting, per-repo rate limit awareness, and configurable concurrency
- **SARIF v2.2 full compliance**: complete SARIF export for security scan results, including rule metadata and suppression entries
- **Config validation**: `aptu config validate` reports missing keys and unknown fields on startup
- **Revert command**: `aptu issue revert <ISSUE>` and `aptu pr revert <PR>` undo all aptu-applied labels and comments on a given issue or PR; builds adopter trust without requiring manual cleanup
- **API key memory hygiene**: apply `zeroize` on drop to all secret-typed fields in `aptu-core`; prevents secrets from lingering in freed memory after deallocation (single-dependency hardening)
- **Claude Max/Pro/Team OAuth**: authenticate via an existing Claude subscription (`credentials.json` from the `claude` CLI) as an alternative to a dedicated API key; eliminates the main onboarding friction point for Anthropic users
- **Prompt caching**: 10-30% cost reduction on active repos, no model switch required. System prompt (5,000 chars) + AST/call-graph context do not change between runs on the same repo. Cache-read cost is 0.1x input cost on both Gemini and Anthropic.

## Medium-Term (6-18 months)

These items require significant design work or external dependencies.

- **Android SDK (KMP)**: expose `aptu-core` to Kotlin via UniFFI-generated bindings; ship an Android companion app for mobile triage review. iOS app is parked indefinitely.
- **Provider health dashboard**: `aptu models list --health` shows real-time availability and latency across configured providers
- **SQLite-backed persistent cache**: replace file-based TTL cache with a SQLite database for faster lookups and cross-session persistence
- **History export**: `aptu history export` in JSON and CSV for personal productivity tracking
- **Multi-forge support**: extend the GitHub API abstractions in `aptu-core` to cover GitLab (cloud + self-managed), Gitea/Forgejo/Codeberg, and Azure DevOps; core triage and review flows work identically across forges
- **Merge queue advisory view**: `aptu pr queue` lists open PRs ranked by a reviewability score (size, age, conflict status, CI result) and highlights next-to-review candidates; advisory only, no auto-merge

## Long-Term (18+ months)

These items are directional signals, not commitments. They depend on the project's maturity and community interest.

- **Multi-LLM orchestration**: route different subtasks (triage summary, label suggestion, complexity assessment) to different models based on cost and capability profiles
- **Independent security audit**: engage a third-party security firm to audit the credential handling, AI prompt injection surface, and SARIF pipeline
- **Structured prompt versioning**: version and test prompts as first-class artifacts alongside source code
- **Federated repo registry**: shared curated repository lists across organizations, with opt-in contribution

## Out of Scope

The following items are deliberately excluded; see [Not Planned](#not-planned) for rationale:

- iOS app
- Gamification and leaderboards
- MCP server (aptu-mcp)

## Not Planned

The following are explicitly out of scope for the foreseeable future:

- A hosted SaaS offering; Aptu is a local CLI and library
- Proprietary model integrations that require closed SDKs
- Automatic merge or code modification; Aptu is advisory only
- Daemon, persistent web dashboard, or TUI; Aptu is a CLI and library, not a server

## Patterns Adopted from aptu-coder

`~/git/clouatre-labs/aptu-coder` was audited for transferable patterns (May 2026). Selected adoptions:

- **Channel-based JSONL observability** (`metrics.rs`): fire metric events into unbounded channel at return; background writer appends to JSONL. Zero blocking on hot path. Applied to the JSONL token-usage artifact (P1, #1225).
- **Path-heuristic relevance filtering** (`test_detection.rs`): skip or deprioritize files by path pattern without parsing. Applied to the docs-only / dependency-bump relevance gate (P1, #1227) and future test-file deprioritization in review.
- **Output-size enforcement** (`output_size` test, `SIZE_LIMIT` constant): enforce token budget at test time, not only at runtime. Worth adopting in `provider.rs` tests.
- **Graceful degradation via `lock_or_recover`** (`cache.rs`): on poisoned mutex, clear and continue rather than panic. Applicable to aptu's disk cache layer.

Patterns audited and not adopted:

- Summary-first cursor-paginated output: aptu is a CLI/Action, not an MCP server; streaming pagination does not apply to single-run AI calls.
- Per-language AST extractors: aptu already has its own AST context pipeline in `provider.rs`.

## Removed from Roadmap

- **iOS App**: not aligned with GitHub Actions / App focus.
- **Gamification / Leaderboards**: deferred; requires platform and user base first.
- **MCP Server** (`aptu-mcp`): removed (see #1232).

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
| #1230 | Prompt caching (Gemini / Anthropic) | P2 |
| #94 | GitHub App | P94 |
