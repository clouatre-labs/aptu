# Aptu GitHub Action

Automatically triage new issues in your repository using the Aptu GitHub Action. The action runs when issues are opened and posts AI-powered analysis and suggestions.

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
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

2. Add your OpenRouter API key as a repository secret (`OPENROUTER_API_KEY`)

## Inputs

- **github-token** (required) - GitHub token for API access (use `secrets.GITHUB_TOKEN`)
- **openrouter-api-key** (required) - OpenRouter API key for AI analysis
- **model** (optional) - OpenRouter model to use (default: `mistralai/devstral-2512:free`)
- **skip-labeled** (optional) - Skip triage if issue already has labels (default: `true`)
- **dry-run** (optional) - Run without making changes (default: `false`)
- **apply-labels** (optional) - Apply AI-suggested labels and milestone (default: `true`)
- **no-comment** (optional) - Skip posting triage comment to GitHub (default: `false`)

## Examples

### Basic Setup

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

### Custom Model

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    model: mistralai/devstral-2512:free
```

### Dry Run (Preview Only)

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    dry-run: true
```

### Skip Already-Labeled Issues

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    skip-labeled: true
```

### Labels Only (No Comment)

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    no-comment: true
```

## Permissions

The action requires the following permissions:

- `issues: write` - To post comments and apply labels
- `contents: read` - To read repository contents

## Environment Variables

You can also configure the action using environment variables:

```yaml
- uses: clouatre-labs/aptu@v0
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
  with:
    model: mistralai/devstral-2512:free
```
