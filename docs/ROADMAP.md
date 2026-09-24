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

- **GitHub App** (#94): `aptu-dev` GitHub App with config-as-code opt-in, mention commands, automatic security scanning, per-installation quotas, and per-repository AI key model. Installed from [github.com/apps/aptu-dev](https://github.com/apps/aptu-dev).
- **Model-tier routing** (#1416): routes large PRs to a higher-capability model tier automatically based on estimated prompt size
- **Prompt optimisation** (#1415): minified schemas, examples moved to user turn (~2.6k chars saved per call)
- **File-based TTL cache eviction** (#1172): `[cache]` config now supports per-field TTL settings (`issue_ttl_minutes`, `repo_ttl_hours`, `file_eviction_days`); stale cache entries are automatically pruned on startup.
- **Per-task AI timeouts** (#1682): `timeout_seconds` is configurable per task via `[ai.tasks.<task>]`, extending the default request timeout for long-running reviews.
- **Structured review verdict/severity badges** (#1683, #1685): PR review comments render verdict and severity badges for machine-scannable findings.
- **Claude Max/Pro/Team OAuth**: authenticate via an existing Claude subscription (`credentials.json` from the `claude` CLI) as an alternative to a dedicated API key.
- **Prompt caching**: automatic on Gemini and Anthropic; system prompt and repo context are cache-eligible, cutting cost on active repos with no model switch required.

## Near-Term (next 3-6 months)

These items address known gaps and complete features already partially implemented.

- **Bulk triage improvements**: better progress reporting, per-repo rate limit awareness, and configurable concurrency
- **SARIF v2.2 full compliance**: complete SARIF export for security scan results, including rule metadata and suppression entries
- **Config validation**: `aptu config validate` reports missing keys and unknown fields on startup
- **API key memory hygiene**: apply `zeroize` on drop to all secret-typed fields in `aptu-core`; prevents secrets from lingering in freed memory after deallocation (single-dependency hardening)
- **Deterministic issue linting** (#1702): `aptu lint-issue` validates issue bodies against template headings and readiness checks with no AI call, plus GitHub Action inputs and a `aptu/lint-issue` commit-status demo workflow

## Medium-Term (6-18 months)

These items require significant design work or external dependencies.

- **Android SDK (KMP)**: expose `aptu-core` to Kotlin via UniFFI-generated bindings; ship an Android companion app for mobile triage review. iOS app is parked indefinitely.
- **Provider health dashboard**: real-time availability and latency reporting across configured providers, sourced from the provider registry cache
- **SQLite-backed persistent cache**: replace file-based TTL cache with a SQLite database for faster lookups and cross-session persistence
- **History export**: JSON and CSV export of the local `history.json` contribution log for personal productivity tracking
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

## Removed from Roadmap

- **iOS App**: not aligned with GitHub Actions / App focus.
- **Gamification / Leaderboards**: deferred; requires platform and user base first.
- **MCP Server** (`aptu-mcp`): removed (see #1232).
