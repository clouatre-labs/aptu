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
* [`aptu pr label`↴](#aptu-pr-label)
* [`aptu pr queue`↴](#aptu-pr-queue)
* [`aptu completion`↴](#aptu-completion)
* [`aptu completion generate`↴](#aptu-completion-generate)
* [`aptu completion install`↴](#aptu-completion-install)
* [`aptu scan-security`↴](#aptu-scan-security)
* [`aptu lint-issue`↴](#aptu-lint-issue)

## `aptu`

CLI for Aptu - Gamified OSS issue triage with AI assistance

**Usage:** `aptu [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `auth` — Manage GitHub authentication
* `issue` — Work with GitHub issues
* `pr` — Work with pull requests
* `completion` — Generate or install shell completion scripts
* `scan-security` — Scan a file or directory for security issues
* `lint-issue` — Lint an issue body file against template headings

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
* `--since <SINCE>` — Triage all issues without labels created since this date (YYYY-MM-DD or RFC3339 format)
* `-s`, `--state <STATE>` — Filter issues by state when using --since (open, closed, or all)

  Default value: `open`

  Possible values:
  - `open`:
    Only open issues (default)
  - `closed`:
    Only closed issues
  - `all`:
    Both open and closed issues

* `--dry-run` — Preview triage without posting to GitHub
* `--no-apply` — Skip applying AI-suggested labels and milestone to the issue
* `--no-comment` — Skip posting triage comment to GitHub
* `-f`, `--force` — Bypass 'already triaged' detection



## `aptu pr`

Work with pull requests

**Usage:** `aptu pr <COMMAND>`

###### **Subcommands:**

* `review` — Review a pull request with AI assistance
* `label` — Auto-label a pull request based on conventional commit prefix and file paths
* `queue` — List and rank open pull requests by reviewability



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
* `--repo-path <PATH>` — Path to the local repository root for AST context injection. Optional when running from within the repo
* `--instructions-file <PATH>` — Path to repository instructions file (overrides default AGENTS.md and .github/instructions/pr-review.md)



## `aptu pr label`

Auto-label a pull request based on conventional commit prefix and file paths

**Usage:** `aptu pr label [OPTIONS] <REFERENCE>`

###### **Arguments:**

* `<REFERENCE>` — PR reference (URL, owner/repo#number, or number)

###### **Options:**

* `-r`, `--repo <REPO>` — Repository for bare PR numbers (e.g., "block/goose")
* `--dry-run` — Preview labels without applying



## `aptu pr queue`

List and rank open pull requests by reviewability

Fetches open PRs for a repository and ranks them by a composite score: 60% size (smaller = higher priority) + 40% age (older = higher priority). Draft PRs are excluded from the ranking (not ready for review).

Default limit is 10; use --limit 0 to show all.

**Usage:** `aptu pr queue [OPTIONS]`

###### **Options:**

* `-r`, `--repo <REPO>` — Repository in owner/repo format (inferred from git if not provided)
* `--limit <LIMIT>` — Maximum number of PRs to display (0 = no limit)

  Default value: `10`



## `aptu completion`

Generate or install shell completion scripts

**Usage:** `aptu completion <COMMAND>`

###### **Subcommands:**

* `generate` — Generate completion script for a shell (output to stdout)
* `install` — Install completion script to standard location



## `aptu completion generate`

Generate completion script for a shell (output to stdout)

**Usage:** `aptu completion generate <SHELL>`

EXAMPLES

  bash
    Add to ~/.bashrc or ~/.bash_profile:
      eval "$(aptu completion generate bash)"

  zsh
    Generate completion file:
      mkdir -p ~/.zsh/completions
      aptu completion generate zsh > ~/.zsh/completions/_aptu

    Add to ~/.zshrc (before compinit):
      fpath=(~/.zsh/completions $fpath)
      autoload -U compinit && compinit -i

  fish
    Generate completion file:
      aptu completion generate fish > ~/.config/fish/completions/aptu.fish

  PowerShell
    Add to $PROFILE:
      aptu completion generate powershell | Out-String | Invoke-Expression


###### **Arguments:**

* `<SHELL>` — Shell to generate completions for

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




## `aptu completion install`

Install completion script to standard location

**Usage:** `aptu completion install [OPTIONS]`

###### **Options:**

* `--shell <SHELL>` — Shell to install completions for (auto-detected from $SHELL if not provided)

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`

* `--dry-run` — Preview installation without writing files



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



## `aptu lint-issue`

Lint an issue body file against template headings

**Usage:** `aptu lint-issue [OPTIONS] --file <FILE>`

###### **Options:**

* `--file <FILE>` — Path to the issue body markdown file
* `--issue-type <TYPE>` — Issue template type to validate against (for example feature, bug). Required when an explicit --config is supplied; optional otherwise
* `--config <FILE>` — Path to a TOML specs file overriding repo-root and generic specs



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
