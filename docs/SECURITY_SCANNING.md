<!-- SPDX-FileCopyrightText: 2026 Aptu Contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Security Scanning

Aptu includes built-in security pattern detection. Scanning is performed locally using pattern matching; no code is sent to external services.

## Standalone scan

Use `aptu scan-security` to scan a directory or file for security issues:

```bash
# Scan a directory, print text summary
aptu scan-security ./crates

# Output SARIF 2.1.0 for GitHub Code Scanning
aptu scan-security . --output sarif > findings.sarif

# Emit GitHub Actions inline annotations
aptu scan-security . --output github-annotations

# Fail CI on critical or high findings
aptu scan-security crates/ --fail-on critical,high

# Suppress findings under test fixtures
aptu scan-security . --fail-on critical,high --exclude tests/fixtures
```

### Flags

| Flag | Description |
|------|-------------|
| `--output sarif\|github-annotations\|json\|text` | Output format (default: `text`) |
| `--fail-on <severities>` | Exit non-zero when any finding matches; comma-separated list: `critical`, `high`, `medium`, `low` |
| `--exclude <prefix>` | Suppress findings under paths matching this prefix; repeatable |

## GitHub Code Scanning integration

Upload SARIF results to enable Code Scanning alerts and inline diff annotations in your repository.

### Workflow (`scan.yml`)

Create `.github/workflows/scan.yml`:

```yaml
name: Scan

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read
  security-events: write

jobs:
  scan:
    name: Security Scan
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6

      - name: Download aptu
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          APTU_VERSION=$(gh api repos/clouatre-labs/aptu/releases \
            --jq '[.[] | select(.tag_name | startswith("v0."))] | first | .tag_name' \
            | sed 's/^v//')
          ARCHIVE="aptu-cli-${APTU_VERSION}-x86_64-unknown-linux-musl.tar.gz"
          gh release download "v${APTU_VERSION}" -R clouatre-labs/aptu \
            --pattern "${ARCHIVE}" --pattern "${ARCHIVE%.tar.gz}.sha256"
          sha256sum -c "${ARCHIVE%.tar.gz}.sha256"
          gh attestation verify "${ARCHIVE}" -R clouatre-labs/aptu
          tar -xzf "${ARCHIVE}"
          install -m 0755 aptu "$HOME/.local/bin/aptu"
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"

      - name: Run security scan
        # || true ensures the SARIF file is always written so upload-sarif
        # never skips. To gate the workflow on findings, add --fail-on and
        # remove || true, or add a separate blocking job using the CI
        # self-audit gate pattern below.
        run: aptu scan-security . --output sarif > findings.sarif || true

      - name: Upload SARIF report
        uses: github/codeql-action/upload-sarif@0daab03d71ff584ef619d027a3fd9146679c5d84 # v3.35.3
        with:
          sarif_file: findings.sarif
          category: aptu-scan-security
```

## CI self-audit gate

Add a required CI job that fails on critical or high findings in your source tree:

```yaml
scan-self:
  name: Scan Self
  runs-on: ubuntu-24.04
  permissions:
    contents: read
  steps:
    - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6
    - name: Build aptu
      run: cargo build --profile ci -p aptu-cli
    - name: Scan source
      run: >
        ./target/ci/aptu scan-security crates/
        --fail-on critical,high
        --exclude crates/aptu-core/src/security
        --output github-annotations
```

Use `--exclude` to suppress known-safe test fixtures and the security pattern definitions themselves.

## Pattern metadata

Every built-in pattern includes:

- **`remediation`** - Concise, actionable guidance for fixing the detected issue.
- **`authority_url`** - Normative reference: a CWE URL (`https://cwe.mitre.org/data/definitions/{N}.html`) for CWE-tagged patterns, or the OWASP LLM Top 10 URL for prompt-injection patterns.

When output is SARIF, these fields populate `tool.driver.rules[]`:

- `shortDescription` and `fullDescription` from the pattern name and description
- `help.text` and `help.markdown` from `remediation`
- `helpUri` from `authority_url`

This enables IDE integrations and code scanning UIs to surface actionable guidance alongside each finding.

## Privacy

Scanning uses local pattern matching only. Source code never leaves your machine.
