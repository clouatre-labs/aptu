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

### Claude Code

Add to `.mcp.json` in your project root, or to `~/.claude.json` for global access:

```json
{
  "mcpServers": {
    "aptu": {
      "type": "stdio",
      "command": "aptu-mcp",
      "args": ["--read-only"],
      "env": {
        "GITHUB_TOKEN": "${GITHUB_TOKEN}",
        "OPENROUTER_API_KEY": "${OPENROUTER_API_KEY}"
      }
    }
  }
}
```

### Kiro

```json
{
  "mcpServers": {
    "aptu": {
      "type": "stdio",
      "command": "aptu-mcp",
      "args": ["--read-only"],
      "env": {
        "GITHUB_TOKEN": "${env:GITHUB_TOKEN}",
        "OPENROUTER_API_KEY": "${env:OPENROUTER_API_KEY}"
      }
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

The hosted instance holds no credentials and requires no authentication token. Tool calls
that require GitHub or AI keys (`triage_issue`, `review_pr`, etc.) must supply credentials
via per-request HTTP headers (see below).

### Per-request credential forwarding

Header names map from provider environment variable names: `GEMINI_API_KEY` →
`X-Gemini-Api-Key`. Supply only the headers your workflow needs.

**goose** (`~/.config/goose/config.yaml`):

```yaml
extensions:
  aptu:
    type: streamable_http
    uri: https://aptu-mcp.fly.dev/mcp
    env_keys:
      - GITHUB_TOKEN
      - GEMINI_API_KEY
      - OPENROUTER_API_KEY
    headers:
      X-Github-Token: "$GITHUB_TOKEN"
      X-Gemini-Api-Key: "$GEMINI_API_KEY"
      X-Openrouter-Api-Key: "$OPENROUTER_API_KEY"
      X-Groq-Api-Key: "$GROQ_API_KEY"
      X-Cerebras-Api-Key: "$CEREBRAS_API_KEY"
      X-Zenmux-Api-Key: "$ZENMUX_API_KEY"
      X-Zai-Api-Key: "$ZAI_API_KEY"
```

`env_keys` loads secrets from the goose keyring into the env map used for `$VAR`
substitution in header values. Variables already present in the shell environment are
substituted without declaring them in `env_keys`.

**Claude Code** (`.mcp.json` or `~/.claude.json`):

```json
{
  "mcpServers": {
    "aptu": {
      "type": "http",
      "url": "https://aptu-mcp.fly.dev/mcp",
      "headers": {
        "X-Github-Token": "${GITHUB_TOKEN}",
        "X-Gemini-Api-Key": "${GEMINI_API_KEY}",
        "X-Openrouter-Api-Key": "${OPENROUTER_API_KEY}"
      }
    }
  }
}
```

Claude Code expands `${VAR}` in header values from the shell environment. No `env` block
is needed for HTTP connections.

**Kiro**:

```json
{
  "mcpServers": {
    "aptu": {
      "type": "http",
      "url": "https://aptu-mcp.fly.dev/mcp",
      "headers": {
        "X-Github-Token": "${env:GITHUB_TOKEN}",
        "X-Gemini-Api-Key": "${env:GEMINI_API_KEY}",
        "X-Openrouter-Api-Key": "${env:OPENROUTER_API_KEY}"
      }
    }
  }
}
```

Kiro uses `${env:VAR}` syntax for environment variable substitution in headers.

### Self-hosted

With bearer token authentication (recommended for production):

```bash
export MCP_BEARER_TOKEN=$(openssl rand -hex 32)
aptu-mcp --transport http --host 0.0.0.0 --port 8080
```

Without authentication (insecure, development only):

```bash
aptu-mcp --transport http --host 0.0.0.0 --port 8080 --allow-unauthenticated
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

## Authentication (operator)

The HTTP server requires bearer token authentication by default. The server will refuse to start if `MCP_BEARER_TOKEN` is absent or empty, unless `--allow-unauthenticated` is explicitly passed.

Set `MCP_BEARER_TOKEN` before starting the server:

```sh
export MCP_BEARER_TOKEN=$(openssl rand -hex 32)
```

Every incoming HTTP request must then present a matching `Authorization: Bearer <token>` header; requests with a missing or wrong token receive a 401 response.

To disable authentication (insecure, not recommended for production), pass `--allow-unauthenticated` at startup.

The hosted instance at `aptu-mcp.fly.dev` runs with `--allow-unauthenticated` because it handles authentication at the proxy layer (Fly's internal network).

### Fly.io

```sh
fly secrets set MCP_BEARER_TOKEN=$(openssl rand -hex 32) --app aptu-mcp
```

### Self-hosted (Docker / other)

Pass the variable at startup:

```sh
docker run -p 8080:8080 -e MCP_BEARER_TOKEN=<token> aptu-mcp
```

### Client configuration (when bearer auth is enabled)

Once enabled, clients must include an `Authorization` header. Each client uses its own
substitution syntax; the token must be present in the shell environment (or the goose
keyring, if declared under `env_keys`).

**goose:**
```yaml
headers:
  Authorization: "Bearer $MCP_BEARER_TOKEN"
```

**Claude Code:**
```json
"Authorization": "Bearer ${MCP_BEARER_TOKEN}"
```

**Kiro:**
```json
"Authorization": "Bearer ${env:MCP_BEARER_TOKEN}"
```

Add the appropriate line to the `headers` block in your existing client configuration.
