<!-- SPDX-FileCopyrightText: 2024 Clouatre Labs -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Security Scanning

Aptu includes built-in security pattern detection for pull request reviews. Scanning is performed locally using pattern matching, and no code is sent to external services.

## Usage

```bash
# Review PR with automatic security scanning
aptu pr review owner/repo#123

# Output SARIF format for GitHub Code Scanning
aptu pr review owner/repo#123 --output sarif > results.sarif
```

## Privacy

Security scanning uses local pattern matching only. Your code stays on your machine.

## GitHub Code Scanning Integration

Upload SARIF results to enable Code Scanning alerts in your repository:

```bash
gh api repos/OWNER/REPO/code-scanning/sarifs \
  -F sarif=@results.sarif \
  -F ref=refs/heads/BRANCH \
  -F commit_sha=COMMIT_SHA
```

### GitHub Actions Example

```yaml
- name: Run aptu security scan
  run: aptu pr review ${{ github.repository }}#${{ github.event.pull_request.number }} --output sarif > results.sarif

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

## Detected Patterns

Aptu scans for common security anti-patterns including:

- Hardcoded secrets and API keys
- SQL injection vulnerabilities
- Command injection risks
- Insecure cryptographic practices
- Sensitive data exposure
