# Roadmap

_Near-Term (next 3-6 months) | Medium-Term (6-18 months) | Long-Term (18+ months)_

This document describes the project direction across three time horizons. Items are based on open issues, the project specification, and known user needs. Dates are approximate and depend on maintainer availability.

## Recently Shipped

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
- **GitHub App support**: enables PRs from forks and org-wide installation without per-repo token management -- the primary blocker for team adoption

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
