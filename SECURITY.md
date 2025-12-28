# Security Policy

## Reporting

Please report issues privately via GitHub's private vulnerability reporting feature or email security@aptu.dev.

Do not open public issues for sensitive matters.

## Credential Storage

Aptu stores tokens in your system keychain (macOS Keychain, Linux Secret Service, Windows Credential Manager). Tokens are never stored in plaintext.

## Best Practices

- Review AI-generated content before posting
- Use `--dry-run` to preview without posting
- Keep Aptu updated

## Supply Chain Security

### SLSA Level 3

All releases include SLSA provenance attestations. Verify with:

```bash
gh attestation verify aptu-<target>.tar.gz --owner clouatre-labs
```

### Build Integrity

- **SHA-pinned Actions** - All GitHub Actions pinned to commit SHA
- **Renovate** - Automated dependency updates with security alerts
- **REUSE/SPDX** - Every file has explicit license metadata

### Artifact Signing

All release artifacts (tarballs and .deb packages) are signed with cosign using keyless signing via Sigstore. Verify signatures with:

```bash
cosign verify-blob --bundle aptu-<target>.tar.gz.bundle --certificate-identity-regexp "https://github.com/clouatre-labs/project-aptu" --certificate-oidc-issuer "https://token.actions.githubusercontent.com"
```

This provides cryptographic proof that artifacts were built by the official CI/CD pipeline without requiring key management.
