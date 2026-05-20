# Aptu GitHub Action

AI-powered automation for GitHub issues and pull requests.

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
    runs-on: ubuntu-24.04
    permissions:
      contents: read
      issues: write
      pull-requests: write
    steps:
      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

Add your AI provider API key as a repository secret. No `provider` or `model` input is needed -- the action detects the provider from whichever API key is set, and uses a sensible default model for that provider.

## What the Action Does

The action auto-detects the event type and runs the appropriate commands:

| Event | Commands | Description |
|-------|----------|-------------|
| `issues` | `aptu issue triage` | Analyze issue, suggest labels and milestone, post comment |
| `pull_request` | `aptu pr label` then `aptu pr review` | Classify PR type, apply label, post advisory review comment |
| `schedule` | `aptu issue triage` (batch) | Triage all unlabeled issues since a given date |
| Any (if `scan-path` or `scan-security-diff` set) | `aptu scan-security` | Scan for security issues, output SARIF |
| Any (if `pr-queue: true`) | `aptu pr queue` | Output ranked reviewability list of open PRs |

PR review is non-blocking (`continue-on-error: true`); a failure does not fail the workflow.

## AI Providers

Provide **one** API key. The action detects the provider automatically.

| Provider | Input | Default Model |
|----------|-------|---------------|
| Anthropic | `anthropic-api-key` | (set via `model`) |
| Cerebras | `cerebras-api-key` | (set via `model`) |
| Google Gemini | `gemini-api-key` | `gemini-3.1-flash-lite-preview` |
| Groq | `groq-api-key` | (set via `model`) |
| OpenRouter | `openrouter-api-key` | `mistralai/mistral-small-2603` |
| Z.AI | `zai-api-key` | (set via `model`) |
| ZenMux | `zenmux-api-key` | (set via `model`) |

When no API key is provided the action falls back to `openrouter` / `inception/mercury-2` via the built-in fallback chain. Override with `provider` and `model` inputs. See [Configuration](CONFIGURATION.md) for details.

## Inputs

### Auth

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `github-token` | Yes | - | GitHub token for API access |

### AI Provider API Keys

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `anthropic-api-key` | No | - | Anthropic API key |
| `cerebras-api-key` | No | - | Cerebras API key |
| `gemini-api-key` | No | - | Google Gemini API key |
| `groq-api-key` | No | - | Groq API key |
| `openrouter-api-key` | No | - | OpenRouter API key |
| `zai-api-key` | No | - | Z.AI API key |
| `zenmux-api-key` | No | - | ZenMux API key |

### AI Model Selection

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `provider` | No | `openrouter` | AI provider to use (`anthropic`, `cerebras`, `gemini`, `groq`, `openrouter`, `zai`, `zenmux`) |
| `model` | No | Provider default | Model identifier for the selected provider |
| `fallback-provider` | No | `openrouter` | Provider to try when the primary fails; set to `''` to disable |
| `fallback-model` | No | `inception/mercury-2` | Model for the fallback provider |

### Behavior Flags

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `apply-labels` | No | `true` | Apply AI-suggested labels and milestone |
| `dry-run` | No | `false` | Run without making changes |
| `no-comment` | No | `false` | Skip posting comment to GitHub |
| `skip-labeled` | No | `true` | Skip triage if the issue already has both a `type:` and a `p[0-9]` label |

### Issue Triage

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `reference` | No | `''` | Issue or PR number/URL to process; if empty, uses the event target |
| `since` | No | `''` | Batch triage: only triage issues created on or after this date (ISO 8601) |
| `issue-state` | No | `open` | Batch triage: filter issues by state (`open`, `closed`, `all`) |

### PR Review

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `instructions-file` | No | `''` | Path to instructions file; overrides default `AGENTS.md` / `.github/instructions/pr-review.md` discovery |
| `repo-path` | No | `''` | Local repository root for AST context injection (Rust, Python, Go, Java, TypeScript, TSX, JS, C, C++, C#, Fortran); leave empty to skip. If omitted, aptu infers the repository root from the current working directory. Explicit values override inference. |
| `deep` | No | `false` | Enable cross-file call-graph context (requires `repo-path`). When `repo-path` is available (explicit or inferred from CWD), call graph enrichment is also auto-enabled for reviews where the remaining prompt budget exceeds `min_budget_for_call_graph` (default: 20 000 chars). Setting `deep: true` forces inclusion regardless of budget. |

**Dependency Enrichment:** For PRs that bump dependencies (Renovate, Dependabot, or manual version bumps), aptu automatically fetches upstream GitHub Release notes and includes them in the review context. Controlled by `max_dep_packages` and `max_dep_release_chars` in `[review]` config.

### Security Scan

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `scan-path` | No | `''` | Directory to scan; leave empty to skip |
| `scan-security-diff` | No | `''` | Path to a unified diff file to scan (overrides `scan-path` when set) |

### PR Queue

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `pr-queue` | No | `false` | Output a ranked reviewability list of open PRs |

### Routing (advanced)

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `command` | No | `issue` | Top-level command (`issue` or `pr`); normally auto-detected from event |
| `subcommand` | No | `triage` | Subcommand (`triage`, `label`, or `review`); normally auto-detected |

## Outputs

| Output | Description |
|--------|-------------|
| `input-tokens` | Total input tokens consumed across all AI calls |
| `output-tokens` | Total output tokens consumed across all AI calls |
| `duration-ms` | Total AI call duration in milliseconds |
| `cost-usd` | Estimated cost in USD (provider-dependent; may be empty) |

Token usage is also written to `$RUNNER_TEMP/aptu-token-usage.jsonl` and uploaded as a workflow artifact (`aptu-token-usage-<run-id>`), and summarized in `$GITHUB_STEP_SUMMARY`.

## Permissions

| Permission | Required For |
|------------|--------------|
| `issues: write` | Issue triage (comments, labels) |
| `pull-requests: write` | PR labeling and review (comments, labels) |
| `contents: read` | Repository context (all features) |

## Observability

Two environment variables control optional output files written during PR review:

| Variable | Description |
|----------|-------------|
| `APTU_CONTEXT_FILE` | Path to write a per-review context JSONL. Each record contains `pr_url`, `repo`, `total_chars`, `budget_drops`, and `prompt_chars_final`. Useful for debugging which enrichments were dropped. |
| `APTU_METRICS_FILE` | Path to write per-review token usage JSONL. |

When running via the GitHub Action, both files are set automatically and uploaded as workflow artifacts (`aptu-review-context.jsonl` and `aptu-token-usage.jsonl`).

## Scheduled Batch Triage

Triage all unlabeled issues on a schedule:

```yaml
name: Aptu scheduled triage

on:
  schedule:
    - cron: '0 8 * * 1'   # every Monday at 08:00 UTC

jobs:
  triage:
    runs-on: ubuntu-24.04
    permissions:
      contents: read
      issues: write
    steps:
      - name: Get last week's date
        id: date
        run: echo "since=$(date -d '7 days ago' +%Y-%m-%d)" >> "$GITHUB_OUTPUT"

      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          since: ${{ steps.date.outputs.since }}
          issue-state: open
```

## PR Review with AST Context

Pass the checked-out repository path to inject function signatures and call-graph context into the review prompt:

```yaml
    steps:
      - uses: actions/checkout@v4

      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          repo-path: ${{ github.workspace }}
          deep: 'true'
```

## Security Scanning

Run `aptu scan-security` on push or PR by setting `scan-path`:

```yaml
      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          scan-path: ${{ github.workspace }}
```

To scan only changed files, pass a diff file via `scan-security-diff` instead:

```yaml
      - name: Generate diff
        run: git diff origin/main...HEAD > /tmp/changes.diff

      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          scan-security-diff: /tmp/changes.diff
```

The scan step outputs SARIF and is non-blocking. See [docs/SECURITY_SCANNING.md](SECURITY_SCANNING.md) for the canonical `scan.yml` workflow and CI self-audit gate pattern.
