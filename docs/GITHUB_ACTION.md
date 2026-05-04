# Aptu GitHub Action

AI-powered automation for GitHub issues, pull requests, and releases.

## Quick Start

```yaml
name: Aptu

on:
  issues:
    types: [opened]
  pull_request:
    types: [opened, synchronize]

jobs:
  aptu:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      issues: write
      pull-requests: write
    steps:
      - name: Checkout repository
        uses: actions/checkout@8e8c483db84b4bee98b60c0593521ed34d9990e8 # v6

      - name: Run Aptu
        uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
```

Add your AI provider API key as a repository secret (e.g., `GEMINI_API_KEY`).

## Features

The action auto-detects the event type and runs the appropriate command:

| Event | Command | Description |
|-------|---------|-------------|
| `issues` | `aptu issue triage` | Analyze issue, suggest labels and milestone, post comment |
| `pull_request` | `aptu pr label` | Classify PR type and apply conventional label |

### Feature-Specific Workflows

For granular control, create separate workflow files:

**Issue triage only** (`.github/workflows/issue-triage.yml`):
```yaml
on:
  issues:
    types: [opened]
permissions:
  issues: write
  contents: read
```

**PR labeling only** (`.github/workflows/pr-label.yml`):
```yaml
on:
  pull_request:
    types: [opened, synchronize]
permissions:
  pull-requests: write
  contents: read
```

## AI Providers

Provide **one** API key. The action auto-detects the provider:

| Provider | Input | Default Model |
|----------|-------|---------------|
| OpenRouter | `openrouter-api-key` | `mistralai/mistral-small-2603` |
| Google Gemini | `gemini-api-key` | `gemini-3.1-flash-lite-preview` |
| Groq | `groq-api-key` | `llama-3.3-70b-versatile` |
| Cerebras | `cerebras-api-key` | `qwen-3-32b` |
| Z.AI | `zai-api-key` | `glm-4.5-air` |
| ZenMux | `zenmux-api-key` | `x-ai/grok-code-fast-1` |

Override with `provider` and `model` inputs. See [Configuration](CONFIGURATION.md) for details.

## Inputs

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `github-token` | Yes | - | GitHub token for API access |
| `gemini-api-key` | No | - | Google Gemini API key |
| `openrouter-api-key` | No | - | OpenRouter API key |
| `fallback-provider` | No | `openrouter` | AI provider for the fallback chain (used when primary provider fails) |
| `fallback-model` | No | `inception/mercury-2` | Model for the fallback provider (e.g. inception/mercury-2) |
| `groq-api-key` | No | - | Groq API key |
| `cerebras-api-key` | No | - | Cerebras API key |
| `zai-api-key` | No | - | Z.AI API key |
| `zenmux-api-key` | No | - | ZenMux API key |
| `model` | No | Provider default | Model to use |
| `provider` | No | Auto-detect | Force specific provider |
| `dry-run` | No | `false` | Preview without changes |
| `skip-labeled` | No | `false` | Skip issues with existing labels |
| `apply-labels` | No | `true` | Apply suggested labels (issues) |
| `no-comment` | No | `false` | Skip posting comment (issues) |
| `repo-path` | No | `''` | Local repository root for AST context injection into PR review prompts. When set, changed source files (Rust, Python, Go, Java, TypeScript, TSX, JavaScript, C, C++, C#, Fortran) are analysed and function signatures with call-graph context are appended to the prompt. Leave empty to skip AST context. |
| `deep` | No | `false` | Enable cross-file call-graph context for richer AI analysis. Requires `repo-path` to be set. |
| `since` | No | `''` | Only triage issues created on or after this date (ISO 8601, e.g. `2024-01-01`). Useful for scheduled batch triage. |

## Permissions

| Permission | Required For |
|------------|--------------|
| `issues: write` | Issue triage (comments, labels) |
| `pull-requests: write` | PR labeling (comments, labels) |
| `contents: read` | Repository context (all features) |

## Scheduled Batch Triage

To triage all new issues on a schedule, combine the `on: schedule` trigger with the `since` input:

```yaml
on:
  schedule:
    - cron: '0 8 * * 1'   # every Monday at 08:00 UTC

jobs:
  triage:
    runs-on: ubuntu-24.04
    steps:
      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          repo: owner/repo
          since: ${{ steps.date.outputs.date }}   # set via a prior step
```

The `since` input filters issues to those created after the given date, preventing the action from re-triaging already-processed issues on repeat runs.

## Security Scanning

The composite action handles issue triage and PR review. For standalone security scanning, create a separate workflow using the `aptu scan-security` subcommand.

See [docs/SECURITY_SCANNING.md](SECURITY_SCANNING.md) for the canonical `scan.yml` workflow, CI self-audit gate pattern, and full flag reference.
