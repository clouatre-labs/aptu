<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- SPDX-FileCopyrightText: 2025 Aptu Contributors -->

# aptu-mcp

MCP server for Aptu - AI-Powered Triage Utility.

[![docs.rs](https://img.shields.io/badge/docs.rs-aptu--mcp-66c2a5?style=flat-square&labelColor=555555&logo=docs.rs)](https://docs.rs/aptu-mcp)
[![Core crate](https://img.shields.io/badge/Core-aptu--core-fc8d62?style=flat-square&labelColor=555555&logo=rust)](https://crates.io/crates/aptu-core)
[![REUSE](https://api.reuse.software/badge/github.com/clouatre-labs/aptu)](https://api.reuse.software/info/github.com/clouatre-labs/aptu)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/11662/badge)](https://www.bestpractices.dev/projects/11662)

## Features

- **5 Tools** - triage_issue, review_pr, scan_security, post_triage, post_review
- **2 Prompts** - triage_guide and review_checklist for guided workflows
- **4 Resources** - curated repos, good first issues, config, and repo detail template
- **Dual Transport** - stdio for local editors, HTTP for remote deployments
- **Multiple Providers** - Gemini (default), Cerebras, Groq, `OpenRouter`, `Z.AI`, and `ZenMux`
- **Read-Only Mode** - Use --read-only flag to disable write operations (post_triage, post_review)

## Installation

```bash
cargo install aptu-mcp
```

## Configuration

Add to your MCP client configuration:

```json
{
  "mcpServers": {
    "aptu": {
      "command": "aptu-mcp",
      "args": ["run"],
      "env": {
        "GITHUB_TOKEN": "ghp_...",
        "AI_API_KEY": "sk-or-..."
      }
    }
  }
}
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `GITHUB_TOKEN` | Yes | GitHub personal access token |
| `AI_API_KEY` | Yes | AI provider API key (fallback for all providers) |
| `OPENROUTER_API_KEY` | No | Provider-specific key (takes precedence over `AI_API_KEY`) |
| `RUST_LOG` | No | Logging level (default: `info`) |

## Development

```bash
cargo test -p aptu-mcp
cargo clippy -p aptu-mcp -- -D warnings
cargo fmt -p aptu-mcp --check
```

## Support

For questions and support, visit [clouatre.ca](https://clouatre.ca/about/).

## License

Apache-2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE).
