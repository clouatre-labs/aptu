# Aptu - Rust CLI [Production]

AI-powered OSS issue triage and PR review with contribution history tracking.

Smart defaults (TTY, rate limits, permissions); `--output json` for automation (schema is a contract); KISS; clean refactors over deprecation.

## Stack

Rust 2024 + Tokio + Clap (derive) + Octocrab + multi-provider AI (OpenAI-compatible interface; see `aptu-core::ai::registry`)

## Workspace Crates

- `aptu-cli` - CLI interface (Clap derive); binary: `aptu`
- `aptu-core` - Core library: AI providers, GitHub API, security scanner, triage engine, cache, history, retry, bulk processing
  - `facade/` - High-level CLI/FFI entry points (ai_client, issues, models, pr_create, pr_review, repos, revert)

## Config & Data Paths (XDG)

- `~/.config/aptu/config.toml` - provider, model, defaults, `[prompt]` byte limits, `[review]` budgets
- `~/.config/aptu/repos.toml` - curated repo list
- `~/.local/share/aptu/history.json` - contribution history

## Commands

```
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo deny check advisories licenses
cargo install --path crates/aptu-cli --profile release
```

Cargo profiles in workspace `Cargo.toml`: `release` (size-optimized, LTO, strip) and `ci` (inherits release, faster compile).

## Project-Specific Patterns

### AI & Transport
- All providers share an OpenAI-compatible interface; registry in `aptu-core::ai::registry`; circuit breaker in `aptu-core::ai::circuit_breaker`
- `AiProvider` trait (`ai/provider/mod.rs`) splits operation logic across `provider/{triage,review,label,create,http,parse}.rs`
- User-prompt builders live in `ai/prompts/mod.rs`; do not inline them in provider files
- Exponential backoff retry with `is_retryable_*` helpers in `aptu-core::retry`
- Bulk processing via `aptu-core::process_bulk` (concurrent triage/review with progress callbacks)

### Security
- `aptu scan-security <path>` -- local pattern matching only; no AI call; each `PatternDefinition` carries `remediation` and `authority_url` (CWE/OWASP)
- SARIF output (`--sarif-output <PATH>`) populates `tool.driver.rules[]` with CWE `helpUri`; uploaded to GitHub Code Scanning via the `scan-self` job in `ci.yml`
- CI self-audit gate: `scan-self` job runs `--fail-on critical,high --output github-annotations` on every push/PR
- `PromptConfig` byte caps (`max_issue_body_bytes=32768`, `max_diff_bytes=524288`, `max_commit_message_bytes=4096`) are prompt-injection guards; CLI exits non-zero on breach

### GitHub Integration
- PR review injects AST + call-graph context from GitHub Contents API; multi-language (Rust, Go, Python, TS, JS, C/C++, C#, Java)
- Structural graph context (petgraph-backed BFS blast-radius, depth-capped via `GraphConfig.max_depth` (default: 4 hops), opt-in via `graph` Cargo feature); disk-cached by commit SHA using postcard serialization with atomic tempfile-then-rename writes and a schema-hash header that auto-invalidates stale cache entries on upgrade; `GraphConfig::validate_consistency()` runs at config load time alongside `ReviewConfig::validate_consistency()` and warns when the graph is enabled with `max_depth = 0` or `max_nodes = 0`
- Model-tier routing selects `small_model` or `large_model` based on estimated prompt size
- Review context budgets in `[review]` (`ReviewConfig`): `max_diff_chars` 200k, `max_patch_chars_per_file` 10k; patches exceeding the per-file limit are dropped; `ReviewConfig::validate_consistency()` warns on misconfigured `min_budget_for_call_graph`; `GraphConfig::validate_consistency()` warns on zero `max_depth` or `max_nodes` when graph is enabled; both run at `AppConfig` load time
- Inline comment dedup keys on `(path, line, side)`; `line=None` comments excluded from map; unchanged body skipped; changed body PATCH-updated in place
- GitHub OAuth device flow; credentials stored in OS keyring; no `GITHUB_TOKEN` env var needed

### Prompts & Schemas
- All prompt text in `crates/aptu-core/src/ai/prompts/` as `.md`/`.json`; edit there, not in Rust source
- System prompt capped at 5,000 chars; JSON schema injected in the user turn, not the system turn

### WASM Portability
- `aptu-core` compiles to `wasm32-unknown-unknown` (no default features); OS-dependent code is `#[cfg(not(target_arch = "wasm32"))]`-gated
- Facade functions that require OS I/O carry the same gate; `wasm_unsupported!` macro in `facade/mod.rs` provides stub bodies
- CI job `wasm-check`: `cargo check -p aptu-core --target wasm32-unknown-unknown --no-default-features`; gate all new OS-only code the same way

### Conventions
- Apache-2.0, REUSE-compliant; every source file needs an SPDX header (`SPDX-License-Identifier: Apache-2.0` + `SPDX-FileCopyrightText`); missing headers fail the `reuse` CI job
- cargo-deny for dependency audits (`advisories` + `licenses`)
- Each AI provider requires a `<PROVIDER>_API_KEY` env var; GitHub auth uses OAuth device flow (keyring-backed)
