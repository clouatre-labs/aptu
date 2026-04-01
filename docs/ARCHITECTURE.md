# Aptu Architecture

## Overview

Aptu is a Rust CLI application for AI-assisted GitHub issue triage. The architecture follows a layered design with clear separation of concerns: CLI interface, domain logic, external integrations, and secure credential management.

## Crate Structure

```
aptu/
├── aptu-cli          # CLI entry point, command routing, user I/O
├── aptu-core         # Domain logic, GitHub API, AI providers
│   ├── ai/           # AI provider abstraction and routing
│   ├── github/       # GitHub API integration (Octocrab wrapper)
│   ├── repos/        # Repository discovery and management
│   └── ...           # Config, cache, history, triage logic
├── aptu-ffi          # Swift/Kotlin FFI bindings via UniFFI
└── aptu-mcp          # MCP server for AI-powered triage and review
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

## Key Abstractions

### TokenProvider Trait
Abstracts credential retrieval across platforms. Implementations:
- `CliTokenProvider` - CLI (env vars, gh CLI, keyring)
- `FfiTokenProvider` - iOS keychain via UniFFI
- `MockTokenProvider` - Testing

### AiProvider Trait
Abstracts AI model invocation across multiple providers (Gemini, OpenRouter, Groq, Cerebras, Zenmux, Z.AI). Each provider:
- Implements unified `chat_completion()` interface
- Manages provider-specific API endpoints and authentication
- Handles rate limiting via `backon` retry strategy

### Facade Functions
`aptu-core/facade.rs` provides high-level functions for CLI/FFI:
- `fetch_issues()`, `analyze_issue()`, `post_triage_comment()`
- `review_pr()`, `post_pr_review()`
- `generate_release_notes()`

Each function accepts a `&dyn TokenProvider` for credential resolution.

### Prompt System

System prompts are built from two layers embedded at compile time via `include_str!` in `crates/aptu-core/src/ai/prompts/`:

- **Schema files** (`.json`) - JSON schema that constrains AI response structure
- **Guideline files** (`.md`) - Instructions and examples for each operation

Builder functions (`build_triage_system_prompt`, `build_review_system_prompt`, etc.) in `prompts/mod.rs` are shared between `provider.rs` (runtime) and `tests/prompt_lint.rs` to guarantee tests exercise the same construction logic.

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
└── [cache]
    └── ttl_seconds = 300
```

## Testing Strategy

- **Unit tests**: Standard `#[cfg(test)]` modules in each crate
- **CLI integration tests**: `crates/aptu-cli/tests/cli.rs` using `assert_cmd` (binary invocation, no HTTP mocking)
- **Core integration tests**: `crates/aptu-core/tests/prompt_lint.rs` and `crates/aptu-core/tests/security_integration.rs`
- **Shell integration tests**: `tests/integration.bats` using the bats framework

## Rust Edition & Tooling

- **Edition**: Rust 2024
- **MSRV**: 1.92.0
- **Linting**: Clippy with pedantic warnings
- **Formatting**: Rustfmt
- **Auditing**: Cargo-deny for vulnerabilities and license compliance
