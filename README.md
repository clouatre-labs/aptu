# Aptu

[![crates.io](https://img.shields.io/crates/v/aptu-cli.svg?style=flat-square&color=fc8d62&logo=rust)](https://crates.io/crates/aptu-cli) [![docs.rs](https://img.shields.io/badge/docs.rs-aptu--core-66c2a5?style=flat-square&labelColor=555555&logo=docs.rs)](https://docs.rs/aptu-core) [![REUSE](https://img.shields.io/reuse/compliance/github.com/clouatre-labs/aptu?style=flat-square)](https://api.reuse.software/info/github.com/clouatre-labs/aptu) [![SLSA Level 3](https://img.shields.io/badge/SLSA-Level%203-green?style=flat-square)](https://slsa.dev) [![OpenSSF Best Practices](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fwww.bestpractices.dev%2Fprojects%2F11662.json&query=%24.badge_level&label=OpenSSF%20Best%20Practices&style=flat-square)](https://www.bestpractices.dev/projects/11662)

Aptu is an AI SDLC review harness for GitHub (GitHub App, CLI, and GitHub Action) that assembles structured context before every AI call, so review quality does not depend on which surface you use.

## GitHub App

Install the [Aptu GitHub App](https://github.com/apps/aptu-dev) and add the dispatch handler workflows to enable AI-powered issue triage and PR review across your repositories.

Grant the app access to a repository, then commit a `.github/aptu.yml` file to opt in:

```yaml
version: 1
triage:
  enabled: true
review:
  enabled: true
  paths:
    - "src/**"
    - "crates/**"
    - "!**/*.md"
ai:
  provider: openrouter
  model: google/gemma-4-26b-a4b-it
```

All installations must supply an `ai` block with `provider` and `model`. The provider determines which repository secret the dispatch handler resolves (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, or `GEMINI_API_KEY`).

Each repository must install the dispatch handler workflows from [aptu-github-app](https://github.com/clouatre-labs/aptu-github-app/blob/main/README.md#installation). The Worker mints an operation-scoped installation token and forwards it to the dispatch handlers; installers do not need `APP_ID` or `APP_PRIVATE_KEY` secrets.

**Mention commands:** Comment `@aptu` on an issue or PR to trigger the app manually. The commenter must be a repository collaborator. Mention commands work regardless of whether automatic dispatch is enabled in `.github/aptu.yml`.

**Automatic security scanning:** When `scan.enabled: true` is set in `.github/aptu.yml`, the app runs `aptu scan-security` on every PR push event, uploads SARIF results to GitHub Code Scanning, and posts a commit status. Scanning is local pattern matching only and does not require an `ai` block. See [docs/SECURITY_SCANNING.md](https://github.com/clouatre-labs/aptu/blob/main/docs/SECURITY_SCANNING.md#app-managed-scanning).

**Quotas:** The app enforces per-installation (50 events per event type per 24h) and global (500 per 24h) rate limits. When a quota is exceeded, the webhook returns `429 Too Many Requests` with a `Retry-After` header.

See [docs/GITHUB_APP.md](https://github.com/clouatre-labs/aptu/blob/main/docs/GITHUB_APP.md) for the permissions matrix and install walkthrough, or [docs/GITHUB_ACTION.md](https://github.com/clouatre-labs/aptu/blob/main/docs/GITHUB_ACTION.md#aptu-dev-github-app) for the full configuration schema.

## Features

| Feature | App | CLI | Action |
|---------|-----|-----|--------|
| Config-as-code (`.github/aptu.yml`) | Yes | - | - |
| AI Triage | Yes | Yes | Yes |
| PR Analysis | Yes | Yes | Yes |
| Dependency Enrichment | Yes | Yes | Yes |
| Multiple Providers | Yes | Yes | Yes |
| Prompt Customization | Yes | Yes | Yes |
| Observability | - | Yes | Yes |
| Model-Tier Routing | - | Yes | Yes |
| Issue Discovery | - | Yes | - |
| Multiple Outputs | - | Yes | - |
| Local History | - | Yes | - |
| Claude OAuth | - | Yes | - |

`aptu pr create --diff <file>` applies a patch, commits, and opens a PR. `--deep` (CLI) / `deep: true` (Action) adds AST and cross-file call-graph context to `pr review` prompts. CLI and Action support seven providers: Anthropic, Cerebras, Gemini, Groq, OpenRouter (default), Z.AI, and ZenMux; the App is limited to Anthropic, Gemini, and OpenRouter (BYOK: the dispatch handler maps `ai.provider` to one of three fixed repository-secret names). Claude OAuth authenticates via `~/.claude/credentials.json` (written by the Claude desktop app); no API key required.

## Architecture Benchmark

Head-to-head comparison of `aptu+mercury-2` ([Mercury 2](https://openrouter.ai/inception/mercury-2), a small diffusion-based LLM by Inception Labs) vs a raw `claude-opus-4.6` call (no schema, no rubric, no AST context) across 6 fixtures (3 triage, 3 PR review).

| Arm | Quality (mean, /5) | Cost/call | Latency p50 |
|-----|----------------|-----------|-------------|
| aptu+mercury-2 | 4.8/5 | $0.0011 | 1,934 ms |
| raw claude-opus-4.6 | 2.2/5 | $0.0193 | 16,032 ms |

This compares a structured-harness call against an unstructured large-model call with no schema, rubric, or AST context; it illustrates the architecture pattern, not model capability.

aptu+mercury-2 is **17x cheaper** and **8x faster** than a raw `claude-opus-4.6` call, while scoring more than twice as high on the structured rubric. See [docs/BENCHMARKS.md](https://github.com/clouatre-labs/aptu/blob/main/docs/BENCHMARKS.md) for full methodology, fixture breakdown, and C1-C5 scores (n=1 per fixture).

## Demo

![Aptu Demo](https://raw.githubusercontent.com/clouatre-labs/aptu/main/assets/demo.gif)

## CLI and Action Installation

The CLI and GitHub Action are self-managed entry points: install and configure them yourself. For a zero-setup, org-wide rollout, use the [GitHub App](#github-app) instead.

```bash
# Homebrew (macOS/Linux)
brew install clouatre-labs/tap/aptu

# Cargo-binstall (fast)
cargo binstall aptu-cli

# Cargo
cargo install aptu-cli
```

## Quick Start

```bash
aptu auth login            # Authenticate with GitHub
aptu repo list             # List curated repositories
aptu issue list --repo block/goose          # Browse issues
aptu issue triage block/goose#123    # Triage with AI
aptu issue triage block/goose#123 --dry-run  # Preview
aptu history               # View your contributions
```

## Observability

```bash
export APTU_METRICS_FILE=metrics.jsonl
aptu pr review owner/repo#123   # token usage appended to metrics.jsonl per run
```

See [docs/GITHUB_ACTION.md](https://github.com/clouatre-labs/aptu/blob/main/docs/GITHUB_ACTION.md#observability) for full field reference.

The GitHub Action also has an opt-in `telemetry-rollup` input that folds a run's own review context into anonymized, aggregate-only counters; see [Telemetry Rollup](https://github.com/clouatre-labs/aptu/blob/main/docs/GITHUB_ACTION.md#telemetry-rollup).

## Security Scanning

Aptu includes built-in security pattern detection for PR reviews. Scanning is performed locally, and no code is sent to external services.

```bash
aptu pr review owner/repo#123                       # Review with security scanning
aptu scan-security . --sarif-output findings.sarif  # SARIF for GitHub Code Scanning
```

See [docs/SECURITY_SCANNING.md](https://github.com/clouatre-labs/aptu/blob/main/docs/SECURITY_SCANNING.md) for SARIF upload and GitHub integration.

## Prompt Customization

Aptu's built-in system prompts are compiled into the binary as defaults. You can override them per operation at runtime or append project-specific guidance globally.

See [docs/CONFIGURATION.md](https://github.com/clouatre-labs/aptu/blob/main/docs/CONFIGURATION.md#prompt-customization) for file paths, operation names, and examples.

## GitHub Action

Auto-triage new issues with AI using any supported provider.

```yaml
- name: AI issue triage and PR review
  uses: clouatre-labs/aptu@83226816caaec41ee93af5e1ca7c974b76de35ba  # v0.10.10
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

Options: `apply-labels`, `no-comment`, `skip-labeled`, `dry-run`, `model`, `provider`.

See [docs/GITHUB_ACTION.md](https://github.com/clouatre-labs/aptu/blob/main/docs/GITHUB_ACTION.md) for setup and examples.

## Configuration

See [docs/CONFIGURATION.md](https://github.com/clouatre-labs/aptu/blob/main/docs/CONFIGURATION.md) for AI provider setup.

## Models

Use `aptu models list` to discover available models from all configured providers.

### Discovering models

```bash
aptu models list                                # all providers
aptu models list --provider openrouter          # OpenRouter only
```

### Filtering and sorting

| Flag | Description |
|------|-------------|
| `--provider` | Filter to a specific provider |
| `--sort name\|context` | Sort by name or context window size |
| `--min-context N` | Show only models with at least N tokens of context |
| `--filter TEXT` | Filter by name or ID (case-insensitive substring match) |

### Free-tier models

OpenRouter exposes pricing data for each model. Models with zero prompt and completion cost are labeled **free** in the output. Use `--provider openrouter` to browse free models.

## Security

This policy is backed by enforced controls: GPG-signed commits, Developer Certificate of Origin, required code owner review, SLSA Level 3 build provenance, and OpenSSF Best Practices Silver. These are not decorative. They ensure that a named, verified human is accountable for every change that reaches users.

- **SLSA Level 3** - Provenance attestations for all releases
- **REUSE/SPDX** - License compliance for all files
- **Signed Commits** - GPG-signed commits required
- **Dependency Scanning** - Automated updates via Renovate

See [SECURITY.md](https://github.com/clouatre-labs/aptu/blob/main/SECURITY.md) for reporting and verification.

## Architecture

Aptu assembles structured context (AST, cross-file call-graph, security scanner output, and dependency release notes) before any AI call. A prompt-injection byte cap and local-only security scanning ensure no raw source code is sent to external services without explicit review. The GitHub App, CLI, and GitHub Action share a common `aptu-core` library; see [docs/ARCHITECTURE.md](https://github.com/clouatre-labs/aptu/blob/main/docs/ARCHITECTURE.md) for the full crate structure, data flow, and key dependencies.

For the governance model behind Aptu's harness design, see [AI SDLC Governance: Three Layers for Engineering Leaders](https://clouatre.ca/posts/ai-sdlc-governance-stack/).

## Roadmap

See [docs/ROADMAP.md](https://github.com/clouatre-labs/aptu/blob/main/docs/ROADMAP.md) for the project direction across near-term, medium-term, and long-term horizons.

## Contributing

We welcome contributions! See [CONTRIBUTING.md](https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md) for guidelines. See [docs/REPO-STANDARDS.md](https://github.com/clouatre-labs/aptu/blob/main/docs/REPO-STANDARDS.md) for a full artifact map and rationale covering CI workflows, tooling, and security controls.

## License

Apache-2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE).
