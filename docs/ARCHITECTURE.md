# Aptu Architecture

## Overview

Aptu is a Rust CLI application for AI-assisted GitHub issue triage and PR review. The architecture follows a layered design with clear separation of concerns: CLI interface, domain logic, external integrations, and secure credential management.

## Crate Structure

```
aptu/
├── aptu-cli          # CLI entry point, command routing, user I/O
└── aptu-core         # Domain logic, GitHub API, AI providers
    ├── ai/           # AI provider abstraction and routing
    ├── git/          # Patch application, branch management, git utilities
    ├── github/       # GitHub API integration (Octocrab wrapper)
    ├── repos/        # Repository discovery and management
    └── ...           # Config, cache, history, triage logic
```

## Data Flow

```
User Input (CLI)
       |
[aptu-cli] Parse args, validate input
       |
[aptu-core] Execute domain logic (triage, review, create)
       |
       +-- [github/] Fetch issues, post comments, apply labels
       +-- [ai/] Generate AI suggestions via provider
       |
[System Keychain] Retrieve tokens securely
       |
External APIs (GitHub, AI Provider)
       |
Format & Display Output
```

A typical aptu invocation follows this path: the CLI parses the command and fetches PR or issue data from the GitHub API (Octocrab); the core library assembles a prompt with AST and call-graph context; the AI provider returns a structured JSON response; the CLI writes labels or review comments back to GitHub and optionally appends token metrics to a JSONL file.

## Key Abstractions

### TokenProvider Trait
Abstracts credential retrieval across platforms. Implementations:
- `CliTokenProvider` - CLI (env vars, gh CLI, keyring)
- `FfiTokenProvider` - mobile keychain via UniFFI (KMP; Android Keystore via KVault)
- `MockTokenProvider` - Testing

### AiProvider Trait
Abstracts AI model invocation across multiple providers (Gemini, OpenRouter, Groq, Cerebras, Zenmux, Z.AI). Each provider:
- Implements unified `chat_completion()` interface
- Manages provider-specific API endpoints and authentication
- Handles rate limiting via `backon` retry strategy

### PR Review Pipeline
`aptu pr review` assembles the AI prompt in layers, each capped by `ReviewConfig`:

1. Fetch PR diff and metadata via Octocrab
2. Fetch full file content for changed files via GitHub Contents API (capped at `max_full_content_files`, `max_chars_per_file`)
3. Build AST context: function signatures and imports for each changed file using `aptu-coder-core` (supports Rust, Python, Go, Java, TypeScript, TSX, JavaScript, C, C++, C#, Fortran)
4. Build call-graph context: cross-file caller chains for changed functions
5. Dependency enrichment: if the PR bumps dependencies, fetch upstream GitHub Release notes for up to `max_dep_packages` packages and include summaries in context (controlled by `ReviewConfig`)
6. Enforce prompt budget (`max_prompt_chars`): drop sections in order (call graph, AST, full content, diff hunks) until budget is met
7. Post inline review comments via GitHub REST API

The `ReviewContext` struct centralises all enrichment decisions: AST context, call graph, instructions, dependency release notes, and budget enforcement are all managed there before the prompt is assembled. Repo-path is inferred from CWD when not explicitly supplied via `--repo-path`.

### apply_patch_and_push (`aptu-core::git::patch`)

`apply_patch_and_push` drives the full patch-to-PR pipeline:

1. Git version gate (>= 2.39.2, CVE-2023-23946)
2. Patch validation: 50 MB size cap, path-traversal rejection, symlink-mode rejection
3. Security scan via `SecurityScanner::scan_diff()` (bypassable with `--force`)
4. Dry-run apply check (`git apply --check`) before any branch is created
5. Branch creation from `origin/<base>` with collision-resistant naming (date suffix, then hex suffix)
6. Patch application, staging, and commit (with optional DCO `--signoff` and GPG `-S` when `commit.gpgSign=true`)
7. Push to `origin`

Returns the branch name that was pushed, or a `PatchError` variant on any failure.

### Facade Functions
`aptu-core/facade/` is a module directory of high-level entry points for CLI and FFI consumers, one file per concern:

| File | Key exports |
|------|-------------|
| `ai_client.rs` | AI client construction and fallback-chain helpers |
| `issues.rs` | `analyze_issue()`, `fetch_issue_for_triage()`, `post_triage_comment()`, `apply_triage_labels()`, `post_issue()`, `format_issue()` |
| `models.rs` | `list_models()`, `validate_model()` |
| `pr_create.rs` | `create_pr()` |
| `pr_review.rs` | `fetch_pr_for_review()`, `analyze_pr()`, `post_pr_review()`, `label_pr()` |
| `repos.rs` | `fetch_issues()`, `list_curated_repos()`, `add_custom_repo()`, `remove_custom_repo()`, `list_repos()`, `discover_repos()` |
| `revert.rs` | `revert_issue()`, `revert_pr()` |

Each function accepts a `&dyn TokenProvider` for credential resolution. Functions that require OS I/O (keyring, filesystem, process spawning) are `#[cfg(not(target_arch = "wasm32"))]`-gated; the `wasm_unsupported!` macro in `facade/mod.rs` provides uniform stub bodies for the wasm32 target.

### Prompt System

System prompts are built from two layers embedded at compile time via `include_str!` in `crates/aptu-core/src/ai/prompts/`:

- **Schema files** (`.json`) - JSON schema that constrains AI response structure
- **Guideline files** (`.md`) - Instructions and examples for each operation

Builder functions (`build_triage_system_prompt`, `build_review_system_prompt`, etc.) in `prompts/mod.rs` are shared between `ai/provider/` (the runtime module directory, one file per operation) and `tests/prompt_lint.rs` to guarantee tests exercise the same construction logic.

At runtime, two override mechanisms are applied in order:

1. **File override** - If `~/.config/aptu/prompts/<operation>.md` exists, it fully replaces the compiled-in guideline for that operation
2. **Custom guidance** - `AiConfig.custom_guidance` (from `config.toml`) is appended to every system prompt after the tooling context

This means users can tune AI behavior without recompiling, and developers can audit the exact prompts from source.

## Security Boundaries

1. **Token Storage**: Credentials stored in system keychain, never in plaintext config
2. **API Keys**: Passed via environment variables or keychain, never logged
3. **User Isolation**: Each user's config/data in XDG paths (`~/.config/aptu/`, `~/.local/share/aptu/`)
4. **Rate Limiting**: Exponential backoff via `backon` prevents API abuse
5. **Least Privilege**: GitHub OAuth scopes limited to `repo:read`, `issues:write`

## Dependencies

**Core Runtime**: Tokio (async), Clap (CLI), Reqwest (HTTP)
**GitHub**: Octocrab (API client), Secrecy (token handling)
**AI**: Provider-agnostic HTTP via Reqwest
**Storage**: Keyring (credentials), Dirs (XDG paths), Config (TOML parsing)
**Error Handling**: Thiserror (library), Anyhow (application)
**Observability**: Tracing with structured logging

## Configuration

```
~/.config/aptu/config.toml
├── [user]
│   └── default_repo = "owner/repo"
├── [ai]
│   ├── provider = "openrouter"
│   └── model = "mistralai/mistral-small-2603"
├── [ui]
│   └── no_color = false
├── [cache]
│   ├── issue_ttl_minutes = 60
│   ├── repo_ttl_hours = 24
│   └── file_eviction_days = 7
├── [review]
│   ├── max_prompt_chars = 120000
│   ├── max_full_content_files = 10
│   ├── max_chars_per_file = 16000
│   ├── max_diff_chars = 200000
│   └── max_patch_chars_per_file = 10000
└── [prompt]
    ├── max_issue_body_bytes = 32768
    ├── max_diff_bytes = 524288
    └── max_commit_message_bytes = 4096
```

## Testing Strategy

- **Unit tests**: Standard `#[cfg(test)]` modules in each crate
- **CLI integration tests**: `crates/aptu-cli/tests/cli.rs` using `assert_cmd` (binary invocation, no HTTP mocking)
- **Core integration tests**: `crates/aptu-core/tests/prompt_lint.rs` and `crates/aptu-core/tests/security_integration.rs`
- **Shell integration tests**: `tests/integration.bats` using the bats framework
- **WASM portability check**: the `wasm-check` CI job runs `cargo check -p aptu-core --target wasm32-unknown-unknown --no-default-features`; OS-dependent code is `#[cfg(not(target_arch = "wasm32"))]`-gated and replaced with stubs on that target

## Rust Edition & Tooling

- **Edition**: Rust 2024
- **MSRV**: 1.96.0
- **Linting**: Clippy with pedantic warnings
- **Formatting**: Rustfmt
- **Auditing**: Cargo-deny for vulnerabilities and license compliance
