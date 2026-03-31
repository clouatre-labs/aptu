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

## Best Practices

- Review AI-generated content before posting
- Use `--dry-run` to preview without posting
- Keep Aptu updated

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
