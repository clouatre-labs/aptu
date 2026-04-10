# Aptu - Rust CLI [Production]

AI-powered OSS issue triage and PR review with gamification.

Smart defaults (TTY, rate limits, permissions); `--output json` for automation (schema is a contract); KISS; clean refactors over deprecation.

## Stack

Rust 2024 + Tokio + Clap (derive) + Octocrab + multi-provider AI (OpenAI-compatible interface; see `aptu-core::ai::registry`)

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
Bearer token auth for the HTTP endpoint: set `MCP_BEARER_TOKEN` env var; omitting it leaves the endpoint unauthenticated (warning logged). Note: Ensure this is used over HTTPS as the token is sent in plain text.

## Config & Data Paths (XDG)

- `~/.config/aptu/config.toml` - provider, model, defaults
- `~/.config/aptu/repos.toml` - curated repo list
- `~/.config/aptu/security.toml` - security scan rules
- `~/.local/share/aptu/history.json` - contribution history

## Environment Variables

Each AI provider requires a `<PROVIDER>_API_KEY` env var (see `aptu-core::ai::registry` for the full list).

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

Cargo profiles defined in workspace `Cargo.toml`: `release` (size-optimized, LTO, strip) and `ci` (inherits release, faster compile).

## Project-Specific Patterns

- Multi-provider AI: all providers share an OpenAI-compatible interface in `aptu-core::ai`; provider registry in `aptu-core::ai::registry`; circuit breaker in `aptu-core::ai::circuit_breaker`
- Exponential backoff retry with `is_retryable_*` helpers in `aptu-core::retry`
- Rate limit awareness and response caching layer (`aptu-core::cache`)
- Bulk processing via `aptu-core::process_bulk` (concurrent triage/review with progress callbacks)
- Security scanning with pattern-based detection; supports SARIF export; includes prompt-injection gate (`aptu-core::security`)
- Inline PR review comments posted via GitHub REST API (`aptu-core::github::pulls::post_pr_review`)
- PR review injects AST + call-graph context fetched from GitHub Contents API; budgets controlled via `[review]` in `config.toml` (`ReviewConfig`: `max_prompt_chars`, `max_full_content_files`, `max_chars_per_file`); multi-language (Rust, Go, Python, TS, JS, C/C++, C#, Java)
- All prompt text lives in `crates/aptu-core/src/ai/prompts/` as `.md`/`.json` files; edit there, not in Rust source
- System prompt is capped at 5,000 chars; JSON schema is injected in the user turn, not the system turn
- Complexity assessment included in every triage response (`ComplexityLevel` + `ComplexityAssessment` in `aptu-core::ai::types`)
- GitHub OAuth device flow; credentials stored in OS keyring
- Contribution history tracking with progress metrics (`aptu-core::history`)
- Least-privilege: MCP server ships with `--read-only` flag; write operations require explicit opt-in
- Apache-2.0 license, REUSE-compliant with SPDX headers on every source file
- cargo-deny for dependency audits (`advisories` + `licenses`)
- PR merge: `gh pr merge --squash` (no merge queue)
- See SPEC.md for full specification
