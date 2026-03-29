# MCP Server

The aptu MCP server supports stdio (local) and HTTP (remote) transports.

## Local (stdio)

### goose

```json
{
  "mcpServers": {
    "aptu": {
      "command": "aptu-mcp",
      "args": ["--read-only"]
    }
  }
}
```

### Claude Desktop

Add to `~/.config/Claude/claude_desktop_config.json` (macOS/Linux) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "aptu": {
      "command": "/path/to/aptu-mcp",
      "args": ["--read-only"]
    }
  }
}
```

## Hosted Instance

A public read-only instance runs at:

```
https://aptu-mcp.fly.dev/mcp
```

Configure your MCP client to connect directly:

### goose

```json
{
  "mcpServers": {
    "aptu": {
      "url": "https://aptu-mcp.fly.dev/mcp"
    }
  }
}
```

### Claude Desktop

```json
{
  "mcpServers": {
    "aptu": {
      "url": "https://aptu-mcp.fly.dev/mcp"
    }
  }
}
```

**Note:** The hosted instance holds no secrets. Tool calls that require GitHub or AI credentials (`triage_issue`, `review_pr`, etc.) must be made from a client that supplies its own `GITHUB_TOKEN` and AI API key via environment variables. The server returns a credential error if they are absent. Bearer token authentication is tracked in #1013.

## Remote (HTTP) -- Self-hosted

```bash
aptu-mcp --transport http --host 0.0.0.0 --port 8080
```

Connect your MCP client to `https://your-host.example.com/mcp`.

## Docker

```bash
docker build -t aptu-mcp .
docker run -p 8080:8080 \
  -e GITHUB_TOKEN=ghp_... \
  -e OPENROUTER_API_KEY=sk-... \
  aptu-mcp
```

Works with any container platform (Cloud Run, Fly.io, Railway, Render, self-hosted).

## Fly.io Deploy

```bash
# From repo root
fly deploy --config crates/aptu-mcp/fly.toml
```

The app runs with `--read-only` (enforced via `[processes]` in `fly.toml`). No secrets are stored on the server; credentials are supplied per-call by MCP clients.

## Options

Remove `--read-only` to enable write tools (`post_triage`, `post_review`). See [CONFIGURATION.md](CONFIGURATION.md) for environment variables and AI provider setup.
