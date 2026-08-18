<!-- SPDX-FileCopyrightText: 2026 Aptu Contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Security Scanning

Aptu includes built-in security pattern detection. Scanning is performed locally using pattern matching; no code is sent to external services.

## Standalone scan

Use `aptu scan-security` to scan a directory or file for security issues:

```bash
# Scan a directory, print text summary
aptu scan-security ./crates

# Write SARIF 2.1.0 to a file for GitHub Code Scanning
aptu scan-security . --sarif-output findings.sarif

# Emit GitHub Actions inline annotations and write SARIF simultaneously
aptu scan-security . --output github-annotations --sarif-output findings.sarif

# Fail CI on critical or high findings
aptu scan-security crates/ --fail-on critical,high

# Suppress findings under test fixtures
aptu scan-security . --fail-on critical,high --exclude tests/fixtures

# Scan only changed lines in a diff (useful for incremental CI)
git diff HEAD~1 | aptu scan-security --diff -
```

### Flags

| Flag | Description |
|------|-------------|
| `--output github-annotations\|json\|text` | Output format (default: `text`) |
| `--sarif-output <PATH>` | Write SARIF 2.1.0 to this file path (independent of `--output`) |
| `--fail-on <severities>` | Exit non-zero when any finding matches; comma-separated list: `critical`, `high`, `medium`, `low` |
| `--exclude <prefix>` | Suppress findings under paths matching this prefix; repeatable |
| `--diff <path>` | Read a unified diff from stdin (use `-`) or a file path and scan only the changed lines; useful for incremental CI scans |

## GitHub Code Scanning integration

Upload SARIF results to enable Code Scanning alerts and inline diff annotations in your repository.

### Workflow example

Download aptu and scan in a workflow job:

```yaml
jobs:
  scan:
    name: Security Scan
    runs-on: ubuntu-24.04
    permissions:
      contents: read
      security-events: write
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
        # --sarif-output writes the SARIF file independently of --output.
        # || true ensures the upload step always runs even when findings are present.
        run: aptu scan-security . --sarif-output findings.sarif || true

      - name: Upload SARIF report
        uses: github/codeql-action/upload-sarif@0daab03d71ff584ef619d027a3fd9146679c5d84 # v3.35.3
        with:
          sarif_file: findings.sarif
          category: aptu-scan-security
```

## CI self-audit gate

Add a required CI job that fails on critical or high findings and uploads SARIF:

```yaml
scan-self:
  name: Scan Self
  runs-on: ubuntu-24.04
  permissions:
    contents: read
    security-events: write
  steps:
    - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6
    - name: Build aptu
      run: cargo build --profile ci -p aptu-cli
    - name: Scan source
      run: >
        ./target/ci/aptu scan-security crates/
        --output github-annotations
        --sarif-output findings.sarif
        --fail-on critical,high
        --exclude crates/aptu-core/src/security
    - name: Upload SARIF report
      if: always()
      uses: github/codeql-action/upload-sarif@0daab03d71ff584ef619d027a3fd9146679c5d84 # v3.35.3
      with:
        sarif_file: findings.sarif
        category: aptu-scan-security
```

Use `--exclude` to suppress known-safe test fixtures and the security pattern definitions themselves.

## Pattern metadata

Every built-in pattern includes:

- **`remediation`** - Concise, actionable guidance for fixing the detected issue.
- **`authority_url`** - Normative reference: a CWE URL (`https://cwe.mitre.org/data/definitions/{N}.html`) for all patterns, including prompt-injection patterns (CWE-1336).

When output is SARIF, these fields populate `tool.driver.rules[]`:

- `shortDescription` and `fullDescription` from the pattern name and description
- `help.text` and `help.markdown` from `remediation`
- `helpUri` from `authority_url`

This enables IDE integrations and code scanning UIs to surface actionable guidance alongside each finding.

## App-Managed Scanning

When using the `aptu-dev` GitHub App, security scanning can be enabled declaratively in `.github/aptu.yml`:

```yaml
scan:
  enabled: true
  fail-on: critical,high
```

When `scan.enabled: true`, the app automatically runs `aptu scan-security` on every PR push event (`opened`, `synchronize`, `reopened`). The scan workflow:

1. Checks out the PR head commit
2. Runs `aptu scan-security` with the configured `fail-on` severities
3. Uploads SARIF results to GitHub Code Scanning
4. Posts a commit status (`aptu/scan-security`) reflecting the scan outcome

The `aptu-scan-security` repository dispatch event can also be triggered manually for ad-hoc scans. See [docs/GITHUB_ACTION.md](https://github.com/clouatre-labs/aptu/blob/main/docs/GITHUB_ACTION.md#app-managed-security-scanning) for the full app configuration schema.

## Privacy

Scanning uses local pattern matching only. Source code never leaves your machine.
