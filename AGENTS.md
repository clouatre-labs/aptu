# Aptu - Rust CLI [Production]

AI-powered OSS issue triage and PR review with gamification.

Smart defaults (TTY, rate limits, permissions); `--output json` for automation (schema is a contract); KISS; clean refactors over deprecation.

## Stack

Rust 2024 + Tokio + Clap (derive) + Octocrab + multi-provider AI (Gemini, OpenRouter, Groq, Cerebras, Zenmux, Z.AI)

## Workspace Crates

- `aptu-cli` - CLI interface (Clap derive); binary: `aptu`
- `aptu-core` - Core library: AI providers, GitHub API, security scanner, triage engine, cache, history, retry, bulk processing
- `aptu-ffi` - Swift/Kotlin bindings (UniFFI)
- `aptu-mcp` - MCP server (rmcp); binary: `aptu-mcp`

## CLI Subcommands

- `auth` (login/logout/status)
- `repo` (list/discover/add/remove)
- `issue` (list/triage/create)
- `pr` (review/label/create)
- `release` (generate/post)
- `models` (list)
- `completion` (generate/install)
- `agent` (run)
- `history`

### Key flags

- `issue triage`: `--since <date>`, `--repo`, `--state`, `--dry-run`, `--no-apply`, `--no-comment`, `--force`
- `pr review`: `--comment`, `--approve`, `--request-changes`, `--dry-run`, `--force`
- Global: `--output json|text`, `--verbose`, `--no-color`

## MCP Server

Tools: `triage_issue`, `review_pr`, `scan_security`, `post_triage`, `post_review`, `health`
Resources: `aptu://repos`, `aptu://issues`, `aptu://config`
Prompts: `triage_guide`, `review_checklist`
Write tools (`post_triage`, `post_review`) are disabled in `--read-only` mode.

## Config & Data Paths (XDG)

- `~/.config/aptu/config.toml` - provider, model, defaults
- `~/.config/aptu/repos.toml` - curated repo list
- `~/.config/aptu/security.toml` - security scan rules
- `~/.local/share/aptu/history.json` - contribution history

## Environment Variables

`GEMINI_API_KEY`, `OPENROUTER_API_KEY`, `GROQ_API_KEY`, `CEREBRAS_API_KEY`, `ZENMUX_API_KEY`, `ZAI_API_KEY`

GitHub auth uses OAuth device flow (keyring-backed); no `GITHUB_TOKEN` env var required for interactive use.

## Commands

```
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo deny check advisories licenses
cargo install --path crates/aptu-cli --profile release   # aptu binary
cargo install --path crates/aptu-mcp --profile release   # aptu-mcp binary
```

Both binaries install to `~/.cargo/bin/`. Do not install via Homebrew; build from source.

Cargo profiles (defined in workspace `Cargo.toml`):
- `release`: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`
- `ci`: inherits release with `lto = false`, `codegen-units = 16`

## Project-Specific Patterns

- Multi-provider AI: all providers share an OpenAI-compatible interface in `aptu-core::ai`; provider registry in `aptu-core::ai::registry`
- Exponential backoff retry with `is_retryable_*` helpers in `aptu-core::retry`
- Rate limit awareness and response caching layer (`aptu-core::cache`)
- Bulk processing via `aptu-core::process_bulk` (concurrent triage/review with progress callbacks)
- Security scanning with pattern-based detection; supports SARIF export (`aptu-core::security`)
- Inline PR review comments posted via GitHub REST API (`aptu-core::github::pulls::post_pr_review`)
- Complexity assessment included in every triage response (`ComplexityLevel` + `ComplexityAssessment` in `aptu-core::ai::types`)
- GitHub OAuth device flow; credentials stored in OS keyring
- Contribution history tracking with progress metrics (`aptu-core::history`)
- Least-privilege: MCP server ships with `--read-only` flag; write operations require explicit opt-in
- Apache-2.0 license, REUSE-compliant with SPDX headers on every source file
- cargo-deny for dependency audits (`advisories` + `licenses`)
- PR merge: `gh pr merge --squash` (no merge queue)
- See SPEC.md for full specification
