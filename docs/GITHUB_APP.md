# aptu-dev GitHub App

The `aptu-dev` GitHub App automates issue triage, PR review, and security scanning without installing the GitHub Action in each repository. A Cloudflare Worker validates webhook signatures, checks the owner allowlist, reads `.github/aptu.yml` from the target repo, and dispatches `repository_dispatch` events to a central workflow in `clouatre-labs/aptu-github-app`.

For the architectural rationale behind Aptu's review-gate model, see [AI SDLC Governance: Three Layers for Engineering Leaders](https://clouatre.ca/posts/ai-sdlc-governance-stack/).

For the `.github/aptu.yml` configuration schema and field reference, see [GitHub Action documentation](GITHUB_ACTION.md#aptu-dev-github-app).

## Permissions

The App requests the following repository permissions at install time. Each is scoped to the minimum required for its operation.

| Permission | Access | Used for |
|------------|--------|----------|
| Contents | Read and write | Read `.github/aptu.yml` and check out the caller repo. Write is used only to trigger `repository_dispatch`, never to modify repository files. |
| Issues | Read and write | Triage reads issue details, comments, and labels; writes triage labels and comments. |
| Pull requests | Read and write | Read PR metadata and changed files for path filtering; write review comments and summaries. |
| Code scanning alerts | Read and write | Upload SARIF results to the repository. Read access is unused but required by GitHub's permission model. |
| Commit statuses | Read and write | Post the `aptu-scan-security` status to the PR head commit. Read access is unused but required by GitHub's permission model. |
| Metadata | Read | Required by GitHub for all Apps. |

The App never has access to your repository secrets or API keys. The `ai.provider` field in `.github/aptu.yml` determines which repository secret the dispatch handler resolves (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, or `GEMINI_API_KEY`), never in the Worker.

## Owner Allowlist

The Worker enforces a hard `ALLOWED_OWNERS` gate before any event processing. Requests whose `repository.owner.login` (or `organization.login`) does not appear in the allowlist are rejected with `403 Forbidden`. The current allowlist is `clouatre-labs,clouatre`. External organizations must be added to this list by the App operator before the App will process their webhooks.

## Installation Walkthrough

1. **Install the App:** Navigate to [https://github.com/apps/aptu-dev](https://github.com/apps/aptu-dev) and click **Install**. Select the repositories (or organization) you want the App to access. The App owner must add your GitHub account or organization to the `ALLOWED_OWNERS` list before webhooks will be processed.

2. **Copy the dispatch handler workflows:** Copy the three dispatch handler workflow files (`aptu-review.yml`, `aptu-triage.yml`, and `aptu-scan-security.yml`) into `.github/workflows/` in the target repository. See the [aptu-github-app installation guide](https://github.com/clouatre-labs/aptu-github-app/blob/main/README.md#installation) for the workflow templates.

3. **Create the opt-in config:** Add `.github/aptu.yml` to the target repository. Triage and review only activate when this file exists and passes validation. See the [configuration reference](GITHUB_ACTION.md#opt-in-configuration) for the full schema. Minimal example:

   ```yaml
   version: 1

   triage:
     enabled: true

   review:
     enabled: true

   ai:
     provider: openrouter
     model: google/gemma-4-26b-a4b-it
   ```

4. **Configure the AI key secret:** In the target repository, go to **Settings > Secrets and variables > Actions** and add a repository secret named after your chosen provider (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, or `GEMINI_API_KEY`). The secret must contain a valid API key for the specified provider. The Worker never sees the secret value; the dispatch handler resolves it in the caller's Actions runtime.

5. **Verify:** Open a test issue or PR in the target repository. If the config is valid and the feature is enabled, the Worker dispatches the corresponding workflow and returns `204 No Content`. If the config is absent, invalid, or the feature is disabled, the Worker returns `200 OK` with no dispatch. Check the target repository's Actions tab for dispatched workflow runs.

## Quota Model

The Worker enforces two tiers of rate limits, both using a rolling 24-hour window:

- **Per-installation quota:** 50 events per event type (triage, review, scan) per 24-hour rolling window. When exceeded, the webhook returns `429 Too Many Requests` with a `Retry-After` header indicating seconds until the oldest event in the window expires.
- **Global quota:** 500 events across all installations per 24-hour rolling window (configurable via `GLOBAL_QUOTA_LIMIT`). Exhaustion returns `429` with a `Retry-After` header.

Quota counters do not reset at a fixed time; timestamps older than 24 hours are pruned on each request.

## Manual Triggers

Comment `@aptu` on an issue or PR to trigger the App manually. The Worker detects the mention, verifies the commenter is a repository collaborator (any collaborator role), enforces quota, then dispatches the corresponding workflow. The bot's own comments are skipped to prevent self-trigger loops.

Mention commands work regardless of whether automatic dispatch is enabled in `.github/aptu.yml`, but are still subject to the owner allowlist and quota limits.
