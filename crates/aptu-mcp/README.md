<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- SPDX-FileCopyrightText: 2025 Aptu Contributors -->

# aptu-mcp

MCP (Model Context Protocol) server for aptu -- exposes GitHub issue triage,
PR review, and security scanning capabilities to AI assistants.

Built with [RMCP](https://github.com/modelcontextprotocol/rust-sdk) v0.14.

## Features

### Tools (5)

| Tool | Description | Annotations |
|------|-------------|-------------|
| `triage_issue` | Fetch and analyze a GitHub issue for triage using AI | read-only, open-world |
| `review_pr` | Fetch and analyze a GitHub pull request for review using AI | read-only, open-world |
| `scan_security` | Scan a unified diff for security vulnerabilities and secrets | read-only, idempotent |
| `post_triage` | Analyze a GitHub issue and post a triage comment with AI insights | destructive, open-world |
| `post_review` | Analyze a GitHub PR and post a review with AI insights | destructive, open-world |

### Prompts (2)

| Prompt | Description |
|--------|-------------|
| `triage_guide` | Step-by-step guide for triaging a GitHub issue |
| `review_checklist` | Checklist for reviewing a GitHub pull request |

### Resources (3 + 1 template)

| Resource | URI | Description |
|----------|-----|-------------|
| Curated Repositories | `aptu://repos` | List of curated open-source repositories |
| Good First Issues | `aptu://issues` | Good first issues from curated repositories |
| Configuration | `aptu://config` | Current aptu configuration settings |
| Repository Detail | `aptu://repos/{owner}/{name}` | Details for a specific curated repository |

## Usage

### Running the Server

```bash
cargo run --bin aptu-mcp
```

The server communicates over stdio using the MCP protocol.

### Environment Variables

Required:
- `GITHUB_TOKEN` -- GitHub personal access token
- `AI_API_KEY` -- AI provider API key (e.g. OpenRouter)

Optional:
- `RUST_LOG` -- Logging level (default: `info`)

### MCP Client Configuration

```json
{
  "mcpServers": {
    "aptu": {
      "command": "cargo",
      "args": ["run", "--bin", "aptu-mcp"],
      "env": {
        "GITHUB_TOKEN": "ghp_...",
        "AI_API_KEY": "sk-or-..."
      }
    }
  }
}
```

## Development

```bash
cargo test -p aptu-mcp
cargo clippy -p aptu-mcp -- -D warnings
cargo fmt -p aptu-mcp --check
```

## License

Apache-2.0
