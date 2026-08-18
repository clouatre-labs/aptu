# GitHub Action

Use the Aptu GitHub Action to automate issue triage, PR review, and security scanning on GitHub.

## Installation

Install the action in your repository via the GitHub Marketplace:
[https://github.com/marketplace/actions/aptu](https://github.com/marketplace/actions/aptu)

Then reference it in a workflow:

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

API key inputs are optional; see [Security](#security) for the provider precedence order.

## Issue Triage

The action automatically triages issues on `opened` or when triggered manually. Example workflow:

```yaml
name: Aptu issue triage

on:
  issues:
    types: [opened]

jobs:
  triage:
    runs-on: ubuntu-24.04
    permissions:
      contents: read
      issues: write
    steps:
      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

The action will comment on the issue with triage analysis and may apply labels if configured in the repository.

## PR Review

Trigger PR review on `opened`, `synchronize`, or `reopened`:

```yaml
name: Aptu PR review

on:
  pull_request:
    types: [opened, synchronize, reopened]

jobs:
  review:
    runs-on: ubuntu-24.04
    permissions:
      contents: read
      pull-requests: write
    steps:
      - uses: actions/checkout@v4

      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

By default, the action reviews all changed files. Use `deep: true` to inject function signatures and call-graph context (see [PR Review with AST Context](#pr-review-with-ast-context) below).

## Configuration

The action respects optional configuration in `.github/aptu-action.yml`:

```yaml
issue-labels:
  - label: triage-needed
    keywords:
      - api
      - security
  - label: performance
    keywords:
      - slow
      - latency
```

## Security

The action uses a three-level precedence to determine API credentials:

1. Explicit secret inputs (`openrouter-api-key`, `anthropic-api-key`, etc.)
2. Repository secrets (auto-detected: `OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, etc.)
3. GitHub token (fallback for some providers)

If no credentials are available, the action fails with a diagnostic message.

## Token Usage

The action writes token usage to `$GITHUB_STEP_SUMMARY` by default. Example output:

```
| Provider | Input | Output | Cache % | ETU |
|----------|-------|--------|---------|-----|
| OpenRouter | 12,000 | 3,500 | 0% | 7.75 |
```

Token usage is also written to `$RUNNER_TEMP/aptu-token-usage.jsonl` and uploaded as a workflow artifact (`aptu-token-usage-<run-id>`), and summarized in `$GITHUB_STEP_SUMMARY` with columns for ETU and Cache%.

## Permissions

| Permission | Required For |
|------------|--------------|
| `issues: write` | Issue triage (comments, labels) |
| `pull-requests: write` | PR labeling and review (comments, labels) |
| `contents: read` | Repository context (all features) |
| `attestations: read` | SLSA binary verification (`gh attestation verify`) |

## Observability

Two environment variables control optional output files written during PR review:

| Variable | Description |
|----------|-------------|
| `APTU_CONTEXT_FILE` | Path to write a per-review context JSONL. Each record contains `pr_url`, `repo`, `total_chars`, `budget_drops`, `files_truncated`, `truncated_chars_dropped`, and `prompt_chars_final`. Useful for debugging which enrichments were dropped. When set via the Action, the context budget (files reviewed/truncated, chars dropped, budget %) is appended to `$GITHUB_STEP_SUMMARY`. |
| `APTU_METRICS_FILE` | Path to write per-review token usage JSONL. Each record includes `effective_token_units` alongside raw token counts. |

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
    runs-on: ubuntu-24.04-arm  # arm64 runner; ~50% cheaper; aptu ships native arm64 binary
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

## aptu-dev GitHub App

The `aptu-dev` GitHub App automates issue triage and PR review without requiring the action to be installed in each repository. A Cloudflare Worker validates webhook signatures and dispatches `repository_dispatch` events to a central review workflow, keeping triage and review logic decoupled from per-repository configuration.

### Installation

Install the app from [https://github.com/apps/aptu-dev](https://github.com/apps/aptu-dev). Grant it access to the repositories you want to triage and review. The app is opt-in: triage and review only activate when `.github/aptu.yml` exists in the target repository.

### Opt-in Configuration

Create `.github/aptu.yml` to enable issue triage, PR review, or both:

```yaml
version: 1

# Enable issue triage
triage:
  enabled: true

# Enable PR review
review:
  enabled: true
  # Optional: path to custom PR review instructions in this repository
  instructions-file: .github/instructions/pr-review.md
  # Optional: skip dispatch if PR already has any of these labels
  skip-labeled: false
  # Optional: glob patterns for PR review dispatch
  paths:
    - "src/**"
    - "crates/**"
    - "!**/*.md"

# AI provider credentials (all three fields required when present)
ai:
  provider: openrouter
  model: google/gemma-4-26b-a4b-it
  # Name of a repository secret containing the API key (must match ^[A-Z0-9_]+$)
  api-key-secret: OPENROUTER_API_KEY

# Security scanning (runs without an ai block)
scan:
  enabled: true
  fail-on: critical,high
  path: src/
```

### Configuration Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `version` | Yes | integer | Configuration schema version. Must be `1`. |
| `triage.enabled` | No | boolean | Enable automatic issue triage (default: `false`). |
| `review.enabled` | No | boolean | Enable automatic PR review (default: `false`). |
| `review.instructions-file` | No | string | Path to custom PR review instructions within this repository (e.g., `.github/instructions/pr-review.md`). |
| `review.skip-labeled` | No | boolean | Skip PR review dispatch if PR has any labels (default: `false`). |
| `review.paths` | No | string[] | Glob patterns for PR review dispatch. Use `!`-prefixed patterns for exclusions. |
| `ai.provider` | See note | string | AI provider (`openai`, `anthropic`, `gemini`, `openrouter`, `bedrock`). All three `ai` fields are required when the `ai` block is present. |
| `ai.model` | See note | string | Model identifier for your configured AI provider. All three `ai` fields are required when the `ai` block is present. |
| `ai.api-key-secret` | See note | string | Name of a repository secret containing the API key. Must match `^[A-Z0-9_]+$`. All three `ai` fields are required when the `ai` block is present. |
| `scan.enabled` | No | boolean | Enable automatic security scanning on PR push events (default: `false`). Scanning is local pattern matching only and does not require an `ai` block. |
| `scan.fail-on` | No | string | Comma-separated severities that fail the scan (`critical`, `high`, `medium`, `low`). |
| `scan.path` | No | string | Root directory to scan (default: `.`). |

### Configuration Requirements

All installations must supply an `ai` block with `provider`, `model`, and `api-key-secret` in `.github/aptu.yml`:

- The `api-key-secret` must be the exact name of a repository secret containing a valid API key for the specified provider
- If the `ai` block is missing or incomplete, the webhook returns `403 Forbidden` with a diagnostic body explaining the requirement
- Security scanning via `scan.enabled: true` does not require an `ai` block (local pattern matching only)

### Dispatch Behavior

The app dispatches on the following events:

- **Issue Triage**: when an issue is opened (if `triage.enabled: true`)
- **PR Review**: when a PR is opened, updated, or reopened (if `review.enabled: true` and `skip-labeled` is not true, and not all changed files match `review.paths`)

Dispatch is skipped (returns `204 No Content`) in these cases:

- `triage.enabled: false` or `review.enabled: false`
- PR review: all changed files match a pattern in `review.paths`
- PR review: `skip-labeled: true` and the PR has at least one label
- Missing or incomplete `ai` block (returns `403 Forbidden` instead of `204`)

### Webhook Signature Validation

The Cloudflare Worker validates all incoming webhook payloads using HMAC-SHA256 signatures. Invalid signatures are rejected with `401 Unauthorized`. The webhook secret is stored securely in Wrangler as a secret and never logged or exposed.

### Mention Commands

Comment `@aptu triage` on an issue or `@aptu review` on a PR to trigger the app manually. The app responds with a reaction to confirm receipt, then dispatches the corresponding workflow. Mention commands work regardless of whether automatic dispatch is enabled in `.github/aptu.yml`.

### Quota Model

The app enforces two tiers of rate limits:

- **Per-installation quota:** Each installation has a configurable request budget per hour. When exceeded, the webhook returns `429 Too Many Requests` with a `Retry-After` header.
- **Global quota:** A shared budget across all installations protects the central worker from overload. Exhaustion returns `429` with a `Retry-After` header.

Quota counters reset at the top of each hour. The `Retry-After` header value is the number of seconds until the next reset window.

### Auth and Execution Model

The app uses a central workflow architecture:

1. **Cloudflare Worker** receives webhook events, validates HMAC signatures, checks `.github/aptu.yml` configuration, and dispatches `repository_dispatch` events to the central review repository.
2. **Central workflow** (`aptu[bot]`) runs the actual triage and review logic. It checks out the target repository, resolves AI credentials, and posts results back as the `aptu[bot]` GitHub App user.
3. **Repository AI keys:** Each installation provides its own AI API key via a repository secret. The central workflow reads the secret name from `.github/aptu.yml` (`ai.api-key-secret`) and resolves it at runtime.

This model keeps triage and review logic in one place while letting each installation control its own AI provider and credentials.

### App-Managed Security Scanning

When `scan.enabled: true` is set in `.github/aptu.yml`, the app runs `aptu scan-security` on every PR push event:

```yaml
# In .github/aptu.yml
scan:
  enabled: true
  fail-on: critical,high
```

The scan workflow:

1. Checks out the PR head commit
2. Runs `aptu scan-security` with the configured `fail-on` severities
3. Uploads SARIF results to GitHub Code Scanning via `github/codeql-action/upload-sarif`
4. Posts a commit status (`aptu/scan-security`) with the result (success, failure, or neutral if no findings)

The `aptu-scan-security` repository dispatch event can also be triggered manually via the GitHub API for ad-hoc scans.

## Cross-Repo PR Review

Trigger Aptu from another repository using `repository_dispatch`. This lets a central workflow review PRs across multiple repositories without installing the action in each one.

**Receiver workflow** (in the repo where Aptu is installed):

```yaml
name: Aptu cross-repo

on:
  repository_dispatch:
    types: [pr-review]

jobs:
  review:
    runs-on: ubuntu-24.04-arm
    permissions:
      contents: read
      pull-requests: write
    steps:
      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          repo: ${{ github.event.client_payload.repo }}
          pull-number: ${{ github.event.client_payload.pull_number }}
```

**Sender workflow** (in the repository whose PRs you want reviewed):

```yaml
name: Request Aptu review

on:
  pull_request:
    types: [opened, synchronize]

jobs:
  dispatch:
    runs-on: ubuntu-24.04
    permissions:
      contents: read
    steps:
      - name: Dispatch to Aptu repo
        run: |
          gh api repos/your-org/aptu-host/dispatches \
            -f event_type=pr-review \
            -f "client_payload[repo]=${{ github.repository }}" \
            -f "client_payload[pull_number]=${{ github.event.pull_request.number }}"
        env:
          GH_TOKEN: ${{ secrets.APTU_DISPATCH_TOKEN }}
```

The `APTU_DISPATCH_TOKEN` secret must have `repo` scope on the Aptu host repository to send the dispatch event. The `github-token` in the receiver workflow needs `pull-requests: write` on the target repository; use a PAT or GitHub App token if the target is a different organization.

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

The scan step outputs SARIF and is non-blocking. See [docs/SECURITY_SCANNING.md](https://github.com/clouatre-labs/aptu/blob/main/docs/SECURITY_SCANNING.md) for the workflow example and CI self-audit gate pattern.
