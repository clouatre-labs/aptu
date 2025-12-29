# Aptu GitHub Action

Automatically triage new issues, label pull requests, and generate AI-curated release notes in your repository using the Aptu GitHub Action. The action runs on issues, pull requests, and releases to provide AI-powered analysis and suggestions.

## Setup

1. Create a workflow file in your repository (`.github/workflows/triage.yml`):

```yaml
name: Triage New Issues

on:
  issues:
    types: [opened]

jobs:
  triage:
    runs-on: ubuntu-latest
    permissions:
      issues: write
      contents: read
    steps:
      - uses: actions/checkout@8e8c483db84b4bee98b60c0593521ed34d9990e8 # v6

      - name: Run Aptu Triage
        uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
```

2. Add your AI provider API key as a repository secret (e.g., `GEMINI_API_KEY`)

## AI Providers

The action supports all providers available in the CLI. Provide **one** API key:

| Provider | Input | Default Model |
|----------|-------|---------------|
| Google Gemini | `gemini-api-key` | `gemini-3-flash-preview` |
| OpenRouter | `openrouter-api-key` | `mistralai/devstral-2512:free` |
| Groq | `groq-api-key` | `llama-3.3-70b-versatile` |
| Cerebras | `cerebras-api-key` | `qwen-3-32b` |
| Z.AI | `zai-api-key` | `glm-4.5-air` |
| ZenMux | `zenmux-api-key` | `x-ai/grok-code-fast-1` |

For detailed provider setup and model options, see [Configuration](CONFIGURATION.md).

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
| `model` | No | Provider default | Model to use (provider-specific) |
| `provider` | No | - | AI provider to use (gemini, openrouter, groq, cerebras, zai, zenmux) |
| `skip-labeled` | No | `false` | Skip triage if issue already has both type and priority labels |
| `dry-run` | No | `false` | Run without making changes |
| `apply-labels` | No | `true` | Apply AI-suggested labels and milestone |
| `no-comment` | No | `false` | Skip posting triage comment |

> **Note:** At least one API key is required.

## Examples

### Google Gemini (Recommended)

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
```

### OpenRouter with Custom Model

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    model: google/gemini-3-flash-preview
```

### Dry Run (Preview Only)

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
    dry-run: 'true'
```

### Labels Only (No Comment)

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
    no-comment: 'true'
```

### Triage All Issues (Including Already-Labeled)

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
    skip-labeled: 'false'
```

### Explicit Provider and Model Selection

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
    provider: gemini
    model: gemini-3-flash-preview
```

## Release Workflow

To generate AI-curated release notes automatically:

```yaml
name: Generate Release Notes

on:
  release:
    types: [published]

jobs:
  release:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@8e8c483db84b4bee98b60c0593521ed34d9990e8 # v6

      - name: Generate Release Notes
        uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
```

The release step automatically triggers on release events and uses the tag from `github.event.release.tag_name` to generate release notes with `aptu release --repo --update`.

## Permissions

The action requires the following permissions:

- `issues: write` - To post comments and apply labels (for issue triage)
- `contents: write` - To update release notes (for release workflow)
- `contents: read` - To read repository contents
