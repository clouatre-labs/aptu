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
  release:
    types: [published]

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
| `release` | `aptu release --repo --update` | Generate AI-curated release notes |

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

**Release notes only** (`.github/workflows/release-notes.yml`):
```yaml
on:
  release:
    types: [published]
permissions:
  contents: write
```

## AI Providers

Provide **one** API key. The action auto-detects the provider:

| Provider | Input | Default Model |
|----------|-------|---------------|
| Google Gemini | `gemini-api-key` | `gemini-3-flash-preview` |
| OpenRouter | `openrouter-api-key` | `mistralai/devstral-2512:free` |
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

## Permissions

| Permission | Required For |
|------------|--------------|
| `issues: write` | Issue triage (comments, labels) |
| `pull-requests: write` | PR labeling (comments, labels) |
| `contents: write` | Release notes (update release body) |
| `contents: read` | Repository context (all features) |
