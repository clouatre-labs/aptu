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

## Remote (HTTP)

### Hosted Instance

A public read-only instance runs at:

```
https://aptu-mcp.fly.dev/mcp
```

Configure your MCP client:

**goose** (`~/.config/goose/config.yaml`):
```yaml
extensions:
  aptu:
    type: streamable_http
    url: https://aptu-mcp.fly.dev/mcp
```

**Claude Desktop** (`claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "aptu": {
      "url": "https://aptu-mcp.fly.dev/mcp"
    }
  }
}
```

**Security note:** The hosted instance holds no credentials. Tool calls that require GitHub or AI keys (`triage_issue`, `review_pr`, etc.) must be made from a client that supplies its own `GITHUB_TOKEN` and AI API key via environment variables. Bearer token authentication is tracked in #1013.

### Self-hosted

```bash
aptu-mcp --transport http --host 0.0.0.0 --port 8080
```

### Deploy to Fly.io

Tag releases redeploy automatically via the `Deploy MCP Server` GitHub Actions workflow. For manual deploys, run from the repo root:

```bash
fly deploy --config crates/aptu-mcp/fly.toml
```

The app runs with `--read-only` (enforced via `[processes]` in `fly.toml`). No secrets are stored on the server.

**One-time setup** (repo maintainer, already done):
```bash
fly apps create aptu-mcp
fly tokens create deploy -x 999999h --app aptu-mcp
# Store output as FLY_API_TOKEN in GitHub → Settings → Environments → fly-production
```

## Docker

```bash
docker build -t aptu-mcp .
docker run -p 8080:8080 \
  -e GITHUB_TOKEN=ghp_... \
  -e OPENROUTER_API_KEY=sk-... \
  aptu-mcp
```

Works with any container platform (Cloud Run, Fly.io, Railway, Render, self-hosted).

## Options

Remove `--read-only` to enable write tools (`post_triage`, `post_review`). See [CONFIGURATION.md](CONFIGURATION.md) for environment variables and AI provider setup.
