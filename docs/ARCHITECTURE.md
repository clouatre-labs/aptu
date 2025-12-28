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
└── aptu-ffi          # Swift FFI bindings (Phase 2+)
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
├── [auth]
│   └── github_token (via keychain, not file)
├── [ai]
│   ├── provider = "gemini"
│   └── model = "gemini-3-flash-preview"
└── [repos]
    └── curated = ["block/goose", ...]
```

## Testing Strategy

- **Unit tests**: Domain logic in `aptu-core` with mocked providers
- **Integration tests**: CLI commands with wiremock HTTP mocking
- **E2E tests**: Against staging GitHub API (CI only)

## Rust Edition & Tooling

- **Edition**: Rust 2024
- **MSRV**: 1.92.0
- **Linting**: Clippy with pedantic warnings
- **Formatting**: Rustfmt
- **Auditing**: Cargo-deny for vulnerabilities and license compliance
