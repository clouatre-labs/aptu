<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- SPDX-FileCopyrightText: 2026 Aptu Contributors -->

# aptu-core

Core library for Aptu - AI-Powered Triage Utility.

[![docs.rs](https://img.shields.io/badge/docs.rs-aptu--core-66c2a5?style=flat-square&labelColor=555555&logo=docs.rs)](https://docs.rs/aptu-core)
[![CLI crate](https://img.shields.io/badge/CLI-aptu--cli-fc8d62?style=flat-square&labelColor=555555&logo=rust)](https://crates.io/crates/aptu-cli)
<a href="https://api.reuse.software/info/github.com/clouatre-labs/aptu"><img alt="REUSE" src="https://img.shields.io/reuse/compliance/github.com/clouatre-labs/aptu?style=for-the-badge" height="20"></a>
<a href="https://www.bestpractices.dev/projects/11662"><img alt="OpenSSF Best Practices" src="https://img.shields.io/cii/level/11662?style=for-the-badge" height="20"></a>

## Features

- **AI Triage** - Analyze issues with summaries, labels, and contributor guidance
- **PR Review** - AI-powered pull request analysis with full file content and multi-language AST context (Rust, Python, Go, Java, TypeScript, TSX, JavaScript, C, C++, C#, Fortran)
- **Security Scanning** - Built-in security pattern detection with SARIF output
- **Multiple Providers** - `OpenRouter` (default), Cerebras, Groq, Gemini, `Z.AI`, and `ZenMux`
- **GitHub Integration** - Auth, issues, PRs, and GraphQL queries
- **Resilient** - Exponential backoff, circuit breaker, rate limit handling

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
aptu-core = "*"
```

> **Note:** Replace `*` with the [current version on crates.io](https://crates.io/crates/aptu-core) when used in production.

### Optional Features

| Feature | Description |
|---------|-------------|
| `keyring` | Secure token storage using system keyring (macOS Keychain, Linux Secret Service, Windows Credential Manager) |
| `ast-context` | AST and call-graph context injection for PR reviews (Rust, Go, Python, TypeScript, JS, C/C++, C#, Java, Fortran) |

To enable optional features:

```toml
[dependencies]
aptu-core = { version = "*", features = ["keyring"] }
```

## Example

```rust,no_run
use aptu_core::{load_config, AiClient, IssueDetails, ai::AiProvider};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = load_config()?;

    // Create AI client
    let client = AiClient::new(&config.ai.provider, &config.ai)?;

    // Create issue details
    let issue = IssueDetails::builder()
        .owner("block".to_string())
        .repo("goose".to_string())
        .number(123)
        .title("Example issue".to_string())
        .body("Issue description...".to_string())
        .url("https://github.com/block/goose/issues/123".to_string())
        .build();

    // Analyze with AI
    let response = client.analyze_issue(&issue).await?;
    println!("Summary: {}", response.triage.summary);

    Ok(())
}
```

## Modules

- [`ai`](https://docs.rs/aptu-core/latest/aptu_core/ai/) - AI integration and triage analysis
- [`config`](https://docs.rs/aptu-core/latest/aptu_core/config/) - Configuration loading and XDG paths
- [`facade`](https://docs.rs/aptu-core/latest/aptu_core/facade/) - High-level platform-agnostic API
- [`github`](https://docs.rs/aptu-core/latest/aptu_core/github/) - GitHub API and authentication
- [`history`](https://docs.rs/aptu-core/latest/aptu_core/history/) - Contribution history tracking
- [`repos`](https://docs.rs/aptu-core/latest/aptu_core/repos/) - Curated repository list
- [`security`](https://docs.rs/aptu-core/latest/aptu_core/security/) - Security pattern detection and SARIF output

## Benchmarks

Head-to-head comparison of `aptu+mercury-2` vs a raw `claude-opus-4.6` call (no schema, no rubric, no AST context) across 6 fixtures (3 triage, 3 PR review).

| Arm | Quality (mean, /5) | Cost/call | Latency p50 |
|-----|-------------------|-----------|-------------|
| aptu+mercury-2 | 4.8/5 | $0.0011 | 1,934 ms |
| raw claude-opus-4.6 | 2.2/5 | $0.0193 | 16,032 ms |

*Measured across aptu #737, #850, #1094 (triage) and #1091, #1098, #1101 (PR review); n=1 per fixture.*

aptu+mercury-2 is **17x cheaper** and **8x faster** than a raw `claude-opus-4.6` call, while scoring more than twice as high on the structured rubric.

See [docs/BENCHMARKS.md](https://github.com/clouatre-labs/aptu/blob/main/docs/BENCHMARKS.md) for full methodology, fixture breakdown, and C1-C5 scores.

## FAQ

**Q: The install examples use `"*"` -- what version should I pin in production?**

The `"*"` wildcard in documentation examples means "any version" and is used so the docs stay accurate across releases. For production use or library dependencies, always pin to a specific version:

```toml
[dependencies]
aptu-core = "0.4"            # semver-compatible: accepts patch and minor updates
# or for exact pinning:
aptu-core = "=0.4.0"         # exact: only this release
```

Check [crates.io/crates/aptu-core](https://crates.io/crates/aptu-core) for the latest published version.
`aptu-core` follows [Semantic Versioning](https://semver.org): patch releases are bug-fixes only; minor releases may add new APIs but remain backward-compatible with existing usage.

## Support

For questions and support, visit [clouatre.ca](https://clouatre.ca/about/).

## License

Apache-2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE).
