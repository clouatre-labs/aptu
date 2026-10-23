# Aptu Architecture

## Overview

Aptu is an AI-powered issue-triage and PR-review harness for GitHub. The architecture follows a layered design with clear separation of concerns: CLI interface, GitHub App webhook dispatch, domain logic, external integrations, and secure credential management.

## Crate Structure

```text
aptu/
├── aptu-cli          # CLI entry point, command routing, user I/O
└── aptu-core         # Domain logic, GitHub API, AI providers
    ├── ai/           # AI provider abstraction and routing
    ├── github/       # GitHub API integration (Octocrab wrapper)
    └── ...           # Config, cache, triage logic
```

## Data Flow

```text
User Input (CLI)          GitHub Webhook (App)
       |                         |
[aptu-cli] Parse args    [Cloudflare Worker] Validate HMAC,
       |                  check .github/aptu.yml, dispatch
[aptu-core] Execute      repository_dispatch event
       |                         |
       +-- [github/]     [Central Workflow] aptu[bot] runs
       +-- [ai/]         triage/review/scan logic
       |                         |
[System Keychain]         [aptu-core] shared library
       |                         |
External APIs (GitHub, AI Provider)
       |
Format & Display Output
```

A typical aptu invocation proceeds as follows: the CLI parses the command and fetches PR or issue data from the GitHub API (Octocrab); the core library assembles a review prompt with AST and call-graph context (both opt-in via `review.ast_context`, disabled by default); the AI provider returns a structured JSON response; the CLI writes labels or review comments back to GitHub and optionally appends token metrics to a JSONL file.

The GitHub App path: a Cloudflare Worker receives webhook events, validates HMAC signatures, reads `.github/aptu.yml` from the repository, and dispatches `repository_dispatch` events to a central workflow. The central workflow runs the same `aptu-core` triage, review, and scan logic as the CLI, posting results as the `aptu[bot]` GitHub App user.

## Key Abstractions

### TokenProvider Trait

Abstracts credential retrieval across platforms. Implementations:

- `CliTokenProvider` - CLI (env vars, gh CLI, keyring)
- `FfiTokenProvider` - mobile keychain via UniFFI (KMP; Android Keystore via KVault)
- `MockTokenProvider` - Testing

### AiProvider Trait

Abstracts AI model invocation across multiple providers (Gemini, OpenRouter, Z.AI, Anthropic). Each provider:

- Implements unified `chat_completion()` interface
- Manages provider-specific API endpoints and authentication
- Handles rate limiting via `backon` retry strategy
- Model-tier routing selects `small_model` or `large_model` based on estimated prompt size, enabling automatic escalation for large PRs
- When a response is truncated (`finish_reason=length`), the HTTP layer retries the same request with an escalated `max_tokens` budget (capped at a conservative provider-agnostic ceiling) instead of failing

### PR Review Pipeline

`aptu pr review` assembles the AI prompt in layers, each capped by `ReviewConfig`:

1. Fetch PR diff and metadata via Octocrab. If a prior Aptu bot review carries an `<!-- APTU_REVIEW:<sha> -->` marker whose SHA differs from the current head, only the compare-commits range (`last_sha..head`) is fetched instead (incremental review). Any failure along this path (missing or unchanged marker SHA, compare API error, empty range) falls back to the full file set; the incremental path never fails the review
2. Fetch full file content for changed files via GitHub Contents API (capped at `max_full_content_files`, `max_chars_per_file`)
3. Enforce prompt budget (`max_prompt_chars`): drop sections in order (file `full_content` -> file patches) until budget is met
4. Verify findings (one extra small-model call when `review.verify_enabled = true`, default): each candidate finding is checked against the diff; findings with a `drop` verdict or confidence below `review.min_confidence` (default 0.7) are dropped, and info-severity findings are suppressed when `review.suppress_info = true` (default). When verification is disabled, only the local info-suppression filter applies
5. Post inline review comments via GitHub REST API
6. Post or update the review summary as an issue comment: the summary carries an `<!-- APTU_REVIEW:<sha> -->` marker keyed to the PR head SHA. An existing Aptu bot summary for the same head SHA is left untouched (dedup); a summary for a different or unknown SHA is updated in place; a missing summary is created after the review posts

The `ReviewContext` struct centralizes all enrichment decisions: instructions and budget enforcement are all managed there before the prompt is assembled. On incremental reviews, a short prior-findings summary (file plus first line of each prior comment, capped to 2,000 characters within the existing prompt budget) is rendered as a `<prior_findings>` prompt section so the AI only reports new findings.

### Facade Functions

`aptu-core/facade/` is a module directory of high-level entry points for CLI and FFI consumers, one file per concern:

| File | Key exports |
|------|-------------|
| `issues.rs` | `analyze_issue()`, `fetch_issue_for_triage()`, `post_triage_comment()`, `apply_triage_labels()` |
| `pr_review/` | `fetch_pr_for_review()`, `analyze_pr()`, `post_pr_review()`, `label_pr()` |

Each function accepts a `&dyn TokenProvider` for credential resolution. Functions that require OS I/O (keyring, filesystem, process spawning) are `#[cfg(not(target_arch = "wasm32"))]`-gated; the `wasm_unsupported!` macro in `facade/mod.rs` provides uniform stub bodies for the wasm32 target.

Write operations (triage comment, labels, PR review) are gated on viewer permission and yield `WriteOutcome::Skipped` when the viewer lacks write access. Permission lookups are memoized process-wide with a 60s TTL.

AI client construction and fallback-provider resolution live in `ai/client.rs`: the `[ai.fallback]` config section defines an ordered chain of fallback entries (provider name or `{provider, model}`), each with its own client so a failing primary provider can be retried downstream.

### Prompt System

System prompts are built from two layers embedded at compile time via `include_str!` in `crates/aptu-core/src/ai/prompts/`:

- **Schema files** (`.json`) - JSON schema that constrains AI response structure
- **Guideline files** (`.md`) - Instructions and examples for each operation

Builder functions (`build_triage_system_prompt`, `build_review_system_prompt`, etc.) in `prompts/mod.rs` are shared between `ai/provider/` (the runtime module directory, one file per operation) and `tests/prompt_lint.rs` to guarantee tests exercise the same construction logic.

At runtime, two override mechanisms are applied in order:

1. **File override** - If `~/.config/aptu/prompts/<operation>.md` exists, it fully replaces the compiled-in guideline for that operation
2. **Custom guidance** - `AiConfig.custom_guidance` (from `config.toml`) is appended to every system prompt after the tooling context

Users can therefore tune AI behavior without recompiling, and developers can audit the exact prompts from source.

## Security Boundaries

1. **Token Storage**: Credentials stored in system keychain, never in plaintext config
2. **API Keys**: Passed via environment variables or keychain, never logged
3. **User Isolation**: Each user's config/data in XDG paths (`~/.config/aptu/`, `~/.local/share/aptu/`)
4. **Rate Limiting**: Exponential backoff via `backon` prevents API abuse
5. **Least Privilege**: GitHub OAuth scopes limited to `public_repo`, `read:user`

## Dependencies

**Core Runtime**: Tokio (async), Clap (CLI), Reqwest (HTTP)
**GitHub**: Octocrab (API client), Secrecy (token handling)
**AI**: Provider-agnostic HTTP via Reqwest
**Storage**: Keyring (credentials), Dirs (XDG paths), Config (TOML parsing)
**Error Handling**: Thiserror (library), Anyhow (application)
**Observability**: Tracing with structured logging

## Configuration

See [docs/CONFIGURATION.md](https://github.com/clouatre-labs/aptu/blob/main/docs/CONFIGURATION.md) for the full reference including all config sections, defaults, and environment variables.

## Testing Strategy

- **Unit tests**: Standard `#[cfg(test)]` modules in each crate
- **CLI integration tests**: `crates/aptu-cli/tests/cli.rs` using `assert_cmd` (binary invocation, no HTTP mocking)
- **Core integration tests**: `crates/aptu-core/tests/prompt_lint.rs` and `crates/aptu-core/tests/security_integration.rs`
- **Shell integration tests**: `tests/integration.bats` using the bats framework
- **WASM portability check**: the `wasm-check` CI job runs `cargo check -p aptu-core --target wasm32-unknown-unknown --no-default-features`; OS-dependent code is `#[cfg(not(target_arch = "wasm32"))]`-gated and replaced with stubs on that target

## Rust Edition & Tooling

- **Edition**: Rust 2024
- **MSRV**: see `rust-toolchain.toml`
- **Linting**: Clippy with pedantic warnings
- **Formatting**: Rustfmt
- **Auditing**: Cargo-deny for vulnerabilities and license compliance
