<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- SPDX-FileCopyrightText: 2025 Aptu Contributors -->

# aptu-core

Core library for Aptu - AI-Powered Triage Utility.

[![docs.rs](https://img.shields.io/badge/docs.rs-aptu--core-66c2a5?style=flat-square&labelColor=555555&logo=docs.rs)](https://docs.rs/aptu-core)
[![CLI crate](https://img.shields.io/badge/CLI-aptu--cli-fc8d62?style=flat-square&labelColor=555555&logo=rust)](https://crates.io/crates/aptu-cli)
[![REUSE](https://api.reuse.software/badge/github.com/clouatre-labs/aptu)](https://api.reuse.software/info/github.com/clouatre-labs/aptu)

## Features

- **AI Triage** - Analyze issues with summaries, labels, and contributor guidance
- **PR Review** - AI-powered pull request analysis and feedback
- **Multiple Providers** - Gemini (default), Cerebras, Groq, `OpenRouter`, Z.AI, and `ZenMux`
- **GitHub Integration** - Auth, issues, PRs, and GraphQL queries
- **Resilient** - Exponential backoff, circuit breaker, rate limit handling

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
aptu-core = "0.2"
```

### Optional Features

| Feature | Description |
|---------|-------------|
| `keyring` | Secure token storage using system keyring (macOS Keychain, Linux Secret Service, Windows Credential Manager) |

To enable optional features:

```toml
[dependencies]
aptu-core = { version = "0.2", features = ["keyring"] }
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
- [`github`](https://docs.rs/aptu-core/latest/aptu_core/github/) - GitHub API and authentication
- [`history`](https://docs.rs/aptu-core/latest/aptu_core/history/) - Contribution history tracking
- [`repos`](https://docs.rs/aptu-core/latest/aptu_core/repos/) - Curated repository list

## Support

For questions and support, visit [clouatre.ca](https://clouatre.ca/about/).

## License

Apache-2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE).
