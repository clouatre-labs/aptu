# Command-Line Help for `aptu`

This document contains the help content for the `aptu` command-line program.

**Command Overview:**

* [`aptu`↴](#aptu)
* [`aptu auth`↴](#aptu-auth)
* [`aptu auth login`↴](#aptu-auth-login)
* [`aptu auth logout`↴](#aptu-auth-logout)
* [`aptu auth status`↴](#aptu-auth-status)
* [`aptu issue`↴](#aptu-issue)
* [`aptu issue triage`↴](#aptu-issue-triage)
* [`aptu pr`↴](#aptu-pr)
* [`aptu pr review`↴](#aptu-pr-review)
* [`aptu scan-security`↴](#aptu-scan-security)

## `aptu`

CLI for Aptu - Gamified OSS issue triage with AI assistance

**Usage:** `aptu [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `auth` — Manage GitHub authentication
* `issue` — Work with GitHub issues
* `pr` — Work with pull requests
* `scan-security` — Scan a file or directory for security issues

###### **Options:**

* `-o`, `--output <OUTPUT>` — Output format (text, json, sarif, github-annotations)

  Default value: `text`

  Possible values:
  - `text`:
    Human-readable text with colors (default)
  - `json`:
    JSON output for programmatic consumption
  - `sarif`:
    SARIF output for security scanning tools
  - `github-annotations`:
    GitHub Actions annotation output (`::error file=...,line=...,title=...::message`)

* `-v`, `--verbose` — Enable verbose output
* `--provider <PROVIDER>` — Override configured AI provider (e.g., openrouter, anthropic)
* `--model <MODEL>` — Override configured AI model (e.g., gpt-4, claude-sonnet-4-6)



## `aptu auth`

Manage GitHub authentication

**Usage:** `aptu auth <COMMAND>`

###### **Subcommands:**

* `login` — Authenticate with GitHub via OAuth device flow
* `logout` — Remove stored credentials
* `status` — Show current authentication status



## `aptu auth login`

Authenticate with GitHub via OAuth device flow

**Usage:** `aptu auth login`



## `aptu auth logout`

Remove stored credentials

**Usage:** `aptu auth logout`



## `aptu auth status`

Show current authentication status

**Usage:** `aptu auth status`



## `aptu issue`

Work with GitHub issues

**Usage:** `aptu issue <COMMAND>`

###### **Subcommands:**

* `triage` — Triage an issue with AI assistance



## `aptu issue triage`

Triage an issue with AI assistance

**Usage:** `aptu issue triage [OPTIONS] [REFERENCE]...`

###### **Arguments:**

* `<REFERENCE>` — Issue references (URL, owner/repo#number, or number)

###### **Options:**

* `-r`, `--repo <REPO>` — Repository for bare issue numbers (e.g., "block/goose")
* `--dry-run` — Preview triage without posting to GitHub
* `--no-apply` — Skip applying AI-suggested labels and milestone to the issue
* `--no-comment` — Skip posting triage comment to GitHub
* `-f`, `--force` — Bypass 'already triaged' detection



## `aptu pr`

Work with pull requests

**Usage:** `aptu pr <COMMAND>`

###### **Subcommands:**

* `review` — Review a pull request with AI assistance



## `aptu pr review`

Review a pull request with AI assistance

**Usage:** `aptu pr review [OPTIONS] [REFERENCE]...`

###### **Arguments:**

* `<REFERENCE>` — PR references (URL, owner/repo#number, or number)

###### **Options:**

* `-r`, `--repo <REPO>` — Repository for bare PR numbers (e.g., "block/goose")
* `--comment` — Post review as a comment (read-only, no approval)
* `--approve` — Post review with approval
* `--request-changes` — Post review requesting changes
* `--dry-run` — Preview the review without posting
* `--no-apply` — Skip applying labels and milestone to the PR
* `--no-comment` — Skip posting review comment to GitHub
* `--no-dedup-summary` — Disable review summary dedup (always post a new summary comment)
* `-f`, `--force` — Bypass confirmation prompts
* `--instructions-file <PATH>` — Path to repository instructions file (overrides default AGENTS.md and .github/instructions/pr-review.md)



## `aptu scan-security`

Scan a file or directory for security issues

**Usage:** `aptu scan-security [OPTIONS] [PATH]`

###### **Arguments:**

* `<PATH>` — Path to scan (file or directory)

###### **Options:**

* `--diff <FILE>` — Read unified diff from FILE or - for stdin
* `--fail-on <FAIL_ON>` — Fail with exit code 1 if findings match these severities (comma-separated: critical,high,medium,low)
* `--exclude <EXCLUDE>` — Exclude paths matching this prefix (repeatable)
* `--sarif-output <PATH>` — Write SARIF output to this file



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
