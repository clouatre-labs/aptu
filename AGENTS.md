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
- `scan-security` (local pattern scan; no AI)
- `models` (list)
- `completion` (generate/install)
- `agent` (run)
- `history`

### Key flags

- `issue triage`: `--since <date>`, `--repo`, `--state`, `--dry-run`, `--no-apply`, `--no-comment`, `--force`
- `pr review`: `--comment`, `--approve`, `--request-changes`, `--dry-run`, `--force`
- `scan-security`: `--output sarif|github-annotations|json|text`, `--fail-on <severities>`, `--exclude <prefix>`
- Global: `--output json|text`, `--verbose`, `--no-color`

## MCP Server

Tools: `triage_issue`, `review_pr`, `scan_security`, `post_triage`, `post_review`, `health`
Resources: `aptu://repos`, `aptu://issues`, `aptu://config`
Prompts: `triage_guide`, `review_checklist`
Write tools (`post_triage`, `post_review`) are disabled in `--read-only` mode.
Bearer token auth for the HTTP endpoint: set `MCP_BEARER_TOKEN` env var; omitting it leaves the endpoint unauthenticated (warning logged). Note: Ensure this is used over HTTPS as the token is sent in plain text.

## Config & Data Paths (XDG)

- `~/.config/aptu/config.toml` - provider, model, review budgets, prompt byte limits
- `~/.config/aptu/repos.toml` - curated repo list
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

### AI & Transport
- All providers share an OpenAI-compatible interface (`aptu-core::ai`); registry in `aptu-core::ai::registry`; circuit breaker in `aptu-core::ai::circuit_breaker`
- Exponential backoff retry with `is_retryable_*` helpers in `aptu-core::retry`
- Rate limit awareness and response caching (`aptu-core::cache`)
- Bulk processing via `aptu-core::process_bulk` (concurrent triage/review with progress callbacks)

### Security
- `aptu scan-security <path>` walks a directory with local pattern matching; no AI call; each `PatternDefinition` carries `remediation` text and `authority_url` (CWE or OWASP reference)
- SARIF output (`--output sarif`) populates `tool.driver.rules[]` with CWE `helpUri`; upload via `scan.yml` workflow; see `docs/SECURITY_SCANNING.md`
- CI self-audit gate: `scan-self` job in `ci.yml` runs `--fail-on critical,high --output github-annotations` on every push/PR
- Prompt-injection input limits in `[prompt]` (`PromptConfig`: `max_issue_body_bytes=32768`, `max_diff_bytes=131072`, `max_commit_message_bytes=4096`); CLI exits non-zero on breach, MCP returns `ToolExecutionError`

### GitHub Integration
- Inline PR review comments posted via GitHub REST API (`aptu-core::github::pulls::post_pr_review`)
- PR review injects AST + call-graph context from GitHub Contents API; multi-language (Rust, Go, Python, TS, JS, C/C++, C#, Java)
- Review context budgets in `[review]` (`ReviewConfig`: `max_prompt_chars`, `max_full_content_files`, `max_chars_per_file`)
- GitHub OAuth device flow; credentials stored in OS keyring
- Least-privilege: MCP server ships with `--read-only` flag; write operations require explicit opt-in

### Prompts & Schemas
- All prompt text in `crates/aptu-core/src/ai/prompts/` as `.md`/`.json`; edit there, not in Rust source
- System prompt capped at 5,000 chars; JSON schema injected in the user turn, not the system turn
- Complexity assessment in every triage response (`ComplexityLevel` + `ComplexityAssessment` in `aptu-core::ai::types`)

### Conventions
- Apache-2.0, REUSE-compliant; SPDX headers on every source file
- cargo-deny for dependency audits (`advisories` + `licenses`)
- Contribution history tracking with progress metrics (`aptu-core::history`)
- PR merge: `gh pr merge --squash` (no merge queue)
- See SPEC.md for full specification
