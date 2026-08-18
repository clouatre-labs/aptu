# Security Policy

## Reporting

Please report issues privately via GitHub's private vulnerability reporting feature or email security@aptu.dev.

Do not open public issues for sensitive matters.

### Response SLA

| Severity | Triage | Acknowledge | Fix Target | Disclosure |
|----------|--------|-------------|------------|------------|
| Critical | 24h    | 24h         | 14 days    | 90 days after fix |
| High     | 24h    | 48h         | 14 days    | 90 days after fix |
| Medium   | 48h    | 72h         | 30 days    | 90 days after fix |
| Low      | 72h    | 7 days      | 90 days    | Coordinated       |

## Credential Storage

Aptu stores tokens in your system keychain (macOS Keychain, Linux Secret Service, Windows Credential Manager). Tokens are never stored in plaintext.

Claude OAuth tokens are read from `~/.claude/credentials.json` if present. Aptu reads this file but never modifies or deletes it.

## Best Practices

- Review AI-generated content before posting
- Use `--dry-run` to preview without posting
- Keep Aptu updated

## Observability Output Security

The `APTU_CONTEXT_FILE` output contains code snippets from the reviewed PR diff and AST context. It does not include prompt text, AI responses, credentials, or personal data. Treat it with the same access controls as the PR diff itself.

## Hosted App Operational Security

The `aptu-dev` GitHub App runs on Cloudflare Workers with a central GitHub Actions workflow for triage and review execution.

### Incident Ownership

The app operator (clouatre-labs) is responsible for:
- Monitoring webhook delivery and worker health
- Responding to availability incidents within the SLA defined above
- Notifying affected installations of any security-relevant changes

### Webhook Secret Rotation

The webhook secret used for HMAC-SHA256 signature validation is rotated on a regular schedule and immediately upon any suspected compromise. Rotation is performed via Wrangler secret update; the new secret is applied to the GitHub App configuration simultaneously to avoid delivery interruptions.

### Repository Secret Handling

External installations supply AI API keys as repository secrets. The central workflow reads these secrets at runtime via the `secrets` context and never logs, stores, or forwards them. Secret names are validated against `^[A-Z0-9_]+$` before resolution. The workflow uses `secrets: inherit` only for the specific secret named in `.github/aptu.yml`.

## Supply Chain Security

### OpenSSF Best Practices

**OpenSSF Best Practices Silver certified.** Fewer than 1% of open source projects reach this level. See [passing criteria](https://www.bestpractices.dev/projects/11662).

### SLSA Level 3

All releases include SLSA provenance attestations. Verify with:

```bash
gh attestation verify aptu-<target>.tar.gz --owner clouatre-labs
```

### Build Integrity

- **SHA-pinned Actions** - All GitHub Actions pinned to commit SHA
- **Renovate** - Automated dependency updates with security alerts
- **REUSE/SPDX** - Every file has explicit license metadata
- **Fuzzing** - cargo-fuzz targets for parser testing

### Repository Security

- **Secret scanning** - Detects accidentally committed credentials
- **Push protection** - Blocks commits containing secrets
- **Validity checks** - Verifies if detected secrets are active

### Branch Protection

Rulesets enforce signed commits, required status checks, CODEOWNERS review, and strict branch freshness (branches must be up-to-date with main before merging). As a solo-maintained project, multi-reviewer requirements are not practical, which limits the OpenSSF Scorecard Branch-Protection score.

### Artifact Signing

All release artifacts (tarballs and .deb packages) are signed with cosign using keyless signing via Sigstore. Verify signatures with:

```bash
cosign verify-blob --bundle aptu-<target>.tar.gz.bundle --certificate-identity-regexp "https://github.com/clouatre-labs/aptu/" --certificate-oidc-issuer "https://token.actions.githubusercontent.com"
```

This provides cryptographic proof that artifacts were built by the official CI/CD pipeline without requiring key management.

### Reporter Credit

Security reporters are acknowledged by their chosen name or pseudonym in the release notes for the version that includes the fix. If a CVE is assigned, reporters are credited in the GitHub Security Advisory by name or pseudonym as they prefer. Reporters who wish to remain anonymous are always respected. We may also list acknowledged reporters in a HALL_OF_FAME file or dedicated release notes section for significant findings.
