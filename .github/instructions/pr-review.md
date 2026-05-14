# PR Review Instructions

## Scope

Review only what the PR changes. Do not flag issues in files the PR does not touch.

## Workflow files

When reviewing `.github/workflows/` changes:

- Evaluate the full job context, not individual steps in isolation. A step that installs a binary
  and a step that executes it are part of the same job; verify both exist before flagging a
  missing publish or execution command.
- Flag `${{ expression }}` interpolation directly inside `run:` scripts as an injection risk;
  inputs should be passed via `env:` blocks.
- Verify action pins use commit SHAs, not mutable tags.
- Check that `permissions:` blocks are present and minimal.

## action.yml

- New AI steps must be `continue-on-error: true` (advisory, non-blocking).
- API key env blocks must be ordered: alphabetical by provider, then `APTU_AI__*`, then
  feature-specific vars; flag deviations.
- New inputs must appear in the correct labelled section (Auth, API keys, AI model selection,
  Behavior flags, Issue triage, PR review, Scan security, PR queue, Routing); flag inputs added
  outside their section.
- The `provider` and `model` inputs default to empty string (CLI resolves to openrouter /
  mistralai/mistral-small-2603); do not flag missing defaults as a bug.

## Rust crates

- Do not suggest adding dependencies without a clear justification.
- Do not flag `.unwrap()` in test code; it is acceptable there.
- The Claude OAuth path (`AuthMethod`, `AiClient::from_claude_credentials`,
  `AiClient::from_keyring_oauth`) reads `~/.claude/credentials.json` and stores the token in the
  OS keyring. Do not flag the keyring call as redundant; it is intentional.
- All token values must use `SecretString` + `Zeroize`; flag any new token field that does not.
- `PromptConfig` byte caps (`max_issue_body_bytes`, `max_diff_bytes`, `max_commit_message_bytes`)
  are prompt-injection guards; do not suggest removing them.
- SARIF output in `scan-security` must populate `tool.driver.rules[]` with a `helpUri` pointing
  to a CWE or OWASP reference for each rule; flag rules that omit `helpUri`.
- Do not comment on code style that `cargo fmt` or `cargo clippy` would catch automatically;
  those are enforced by CI.

## General

- One comment per distinct issue; do not duplicate findings across multiple inline comments.
- Prefer suggesting a fix (suggestion block) over describing the problem when the fix is
  unambiguous.
