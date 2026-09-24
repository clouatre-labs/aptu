<!-- SPDX-FileCopyrightText: 2026 Aptu Contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Issue Linting

Aptu includes a deterministic, no-AI issue-body validator: `aptu lint-issue`. It checks a GitHub issue body against template headings and readiness signals using local pattern matching only; no model call, no network, no provider keys.

## What it checks

Two modes exist.

With a spec (explicit `--config` or a repo-root `issue-lint-specs.toml`), the body is validated against the matching `[[spec]]` entry:

- every required H2 heading is present (headings are matched after trimming, on lines starting with `## `)
- at least one fenced code block or file-path reference, when `require_code_examples = true`
- at least one external URL or `#N` issue reference, when `require_external_link = true`

All spec checks operate on visible text only: HTML comments and fenced-block contents are stripped before matching, consistent with generic mode.

Without any spec, generic mode applies four deterministic checks derived from issue-readiness research (arXiv 2512.21426, Table I):

1. body length above a one-liner floor (100 characters)
2. an acceptance-criteria-like section (heading containing "acceptance") with at least 2 checkboxes
3. at least one file-path reference or code fence
4. at least one external URL or `#N` issue reference

Generic mode needs zero configuration and covers any repository. It applies regardless of `--issue-type`.

## CLI usage

```bash
# Lint an issue body in generic mode (no config, no --issue-type needed)
aptu lint-issue --file issue-body.md

# Lint an issue body against a repo config
aptu lint-issue --file issue-body.md --issue-type feature

# Explicit config overrides repo-root auto-discovery
aptu lint-issue --file issue-body.md --issue-type feature --config issue-lint-specs.toml

# Emit GitHub Actions annotations (one per violation)
aptu lint-issue --file issue-body.md --issue-type feature --output github-annotations
```

Exit codes:

- `0` pass (including generic-mode pass)
- `1` violations found (output is emitted before the exit so failing steps stay diagnosable)
- `2` an explicitly supplied config is broken/malformed, was given without `--issue-type` (spec matching needs a type), or has no `[[spec]]` matching `--issue-type`; a clear error is printed to stderr

Output formats: `text`, `json`, and `github-annotations`. `sarif` is not supported for `lint-issue` (SARIF applies to `scan-security` only) and is rejected with a clear error.

## Spec resolution

Resolution is automatic, in order:

1. explicit `--config <specs.toml>`
2. `issue-lint-specs.toml` at the repository root (found by walking upward from the working directory until a `.git` entry is located; falls back to the working directory itself)
3. generic mode

`--issue-type` is optional. Generic mode never needs it; a repo-root spec uses it when matching and falls back to generic mode if no spec matches. When an explicit `--config` is supplied, `--issue-type` is required: a spec matches by its `type` field, and an explicit config without `--issue-type` (or with a type the config lacks) is a hard error (exit 2).

Because discovery walks upward to the repository root, the command works from any subdirectory of a checkout, including worktrees. Outside a repository, only a spec file in the working directory itself is considered.

## Config schema

```toml
# issue-lint-specs.toml (repository-supplied config, --config input)
[[spec]]
type = "feature"
required_headings = ["Summary", "Context", "Implementation Notes", "Acceptance Criteria", "Not In Scope"]
require_code_examples = true
require_external_link = true

[[spec]]
type = "bug"
required_headings = ["Summary", "Steps to Reproduce", "Expected Behavior", "Actual Behavior", "Environment"]
```

All fields except `type` are optional; `required_headings` defaults to empty and both `require_*` flags default to `false`.

## Action inputs

The aptu GitHub Action accepts:

- `lint-issue-file`: path to the issue body markdown file; leave empty to skip the lint step
- `lint-issue-type`: template type to validate against (default `feature`)
- `lint-fail-on`: set to `'true'` to fail the workflow on violations; default is advisory (annotations only, `continue-on-error`)

A demo workflow posting the `aptu/lint-issue` commit status lives in `.github/workflows/lint-issue-demo.yml`.

## Limitations

- GitHub issue forms are unsupported in v1: `refactor.yml`-style form-derived bodies do not carry the same H2 structure and are left to triage. Markdown templates only.
- Headings are matched as plain `## ` lines; headings inside fenced code blocks and HTML comments are ignored, as are blockquoted headings.
- No auto-closing or blocking: the operation only reports (text, JSON, or annotations); enforcement (comment/label/status) is the workflow or App layer's job.
- App-managed linting on `issues` events (comment + label) is tracked separately in the aptu-github-app repository.
