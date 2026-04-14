# Roadmap

_Near-Term: Q2 2026 | Medium-Term: Q3–Q4 2026 | Long-Term: 2027+_

This document describes the project direction across three time horizons. Items are based on open issues, the project specification, and known user needs. Dates are approximate and depend on maintainer availability.

## Near-Term (next 3-6 months)

These items address known gaps and complete features already partially implemented.

- **Bulk triage improvements**: better progress reporting, per-repo rate limit awareness, and configurable concurrency
- **SARIF v2.2 full compliance**: complete SARIF export for security scan results, including rule metadata and suppression entries
- **MCP resource paging**: `aptu://issues` resource returns paginated results for large repositories
- **Config validation**: `aptu config validate` reports missing keys and unknown fields on startup

## Medium-Term (6-18 months)

These items require significant design work or external dependencies.

- **iOS and Android SDK**: expose `aptu-core` to Swift and Kotlin via UniFFI-generated bindings; ship a companion app for mobile triage review
- **Web UI**: read-only dashboard backed by the MCP server; no framework dependency, plain HTML and fetch
- **Provider health dashboard**: `aptu models list --health` shows real-time availability and latency across configured providers
- **Persistent cache layer**: SQLite-backed response cache with TTL eviction to reduce redundant AI calls across sessions
- **History export**: `aptu history export` in JSON and CSV for personal productivity tracking

## Long-Term (18+ months)

These items are directional signals, not commitments. They depend on the project's maturity and community interest.

- **Multi-LLM orchestration**: route different subtasks (triage summary, label suggestion, complexity assessment) to different models based on cost and capability profiles
- **Independent security audit**: engage a third-party security firm to audit the credential handling, AI prompt injection surface, and SARIF pipeline
- **Structured prompt versioning**: version and test prompts as first-class artifacts alongside source code
- **Federated repo registry**: shared curated repository lists across organizations, with opt-in contribution

## Not Planned

The following are explicitly out of scope for the foreseeable future:

- A hosted SaaS offering; Aptu is a local CLI and library
- Proprietary model integrations that require closed SDKs
- Automatic merge or code modification; Aptu is advisory only
