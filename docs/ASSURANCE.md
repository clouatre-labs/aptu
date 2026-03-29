# Security Assurance

This document describes the security model of Aptu for the purpose of OpenSSF Best Practices (Silver) criterion compliance. It covers threat actors, trust boundaries, input validation, security controls, and residual risks.

## Threat Model

### Threat Actors

| Actor | Goal | Capability |
|-------|------|-----------|
| Malicious repo maintainer | Inject prompt content via issue or PR body to manipulate AI output | Control repository content fed to AI |
| Compromised AI provider | Return crafted responses that cause Aptu to post malicious comments or apply incorrect labels | Control AI response content |
| MITM attacker | Intercept API traffic to read tokens or modify responses | Network position between client and GitHub/AI APIs |
| Credential thief | Exfiltrate GitHub OAuth tokens or AI API keys | Access to the user's filesystem or OS keyring |

### Attack Surface

Aptu does not execute remote code, evaluate arbitrary expressions, or write to the local filesystem beyond XDG config and data paths. The primary attack surface is:

1. AI prompt content derived from untrusted repository data (issue titles, PR diffs, commit messages)
2. API response parsing (GitHub REST, AI provider OpenAI-compatible APIs)
3. Credential storage and retrieval (OS keyring, environment variables)

## Trust Boundaries

```
+------------------+     TLS 1.2+      +------------------+
|   CLI process    | ----------------> |   GitHub API     |
|  (aptu-cli)      |                   |  (api.github.com)|
|                  |     TLS 1.2+      +------------------+
|                  | ----------------> |   AI Provider    |
|                  |                   |  (varies by cfg) |
|                  |                   +------------------+
|                  |    IPC / syscall  +------------------+
|                  | ----------------> |   OS Keyring     |
|                  |                   | (macOS/Linux/Win)|
|                  |    filesystem     +------------------+
|                  | ----------------> |   Config files   |
+------------------+                   | (~/.config/aptu) |
                                       +------------------+
```

**Boundaries and assumptions:**

- CLI process to GitHub API: TLS enforced by rustls; no plaintext fallback. Responses are parsed with serde; unexpected fields are ignored.
- CLI process to AI provider (OpenRouter, Gemini/Google, Groq, Cerebras, Zenmux, Z.AI): TLS enforced; API keys transmitted only in Authorization headers, never in request bodies or URLs.
- CLI process to OS keyring: platform keyring API (keyring crate); tokens are never written to disk in plaintext.
- Config files: user-owned files in `~/.config/aptu/`; no secrets are stored there (see keyring above).

## Input Validation

### CLI Input

All CLI arguments are parsed by Clap using typed structs with derive macros. Unknown flags are rejected at parse time. String inputs (repo slugs, issue numbers, dates) are validated by Clap type constraints before reaching domain logic.

### API Response Validation

GitHub and AI provider responses are deserialized with serde into typed Rust structs. Deserialization failures return an error; no partial or untyped data propagates to downstream logic. Numeric fields (issue numbers, PR numbers) are bounded by Rust's `u64` type.

### Prompt Construction

Issue and PR content is inserted into AI prompts as quoted strings. Aptu does not execute or evaluate any content from issue bodies, PR diffs, or commit messages. Prompt injection may cause unexpected AI output but cannot cause Aptu to execute code or access unauthorized resources.

## Security Controls

| Control | Implementation | Status |
|---------|---------------|--------|
| TLS 1.2+ for all outbound connections | rustls (no OpenSSL dependency) | Active |
| Credential storage | OS keyring via `keyring` crate; no plaintext on disk | Active |
| Release artifact signing | cosign (keyless, Sigstore) via GitHub Actions OIDC | Active |
| SLSA Level 3 provenance | GitHub Actions provenance attestation | Active |
| Dependency vulnerability scanning | `cargo deny check advisories` in CI | Active |
| License compliance | REUSE 3.x, verified by `reuse lint` in CI | Active |
| Branch protection | Requires passing CI; squash merge only; signed commits required | Active |
| Signed commits | GPG-signed, DCO sign-off enforced | Active |
| Security disclosure process | Private reporting via GitHub Security Advisories | Active |
| Dependency pinning | Actions pinned to SHA; `cargo deny` blocks unmaintained crates | Active |

## Residual Risks

The following risks are acknowledged and accepted for the current project maturity:

1. **AI provider trust**: Aptu trusts the configured AI provider to return well-formed responses. A compromised provider could return content designed to appear as a legitimate triage result. Mitigation: responses are parsed by typed structs; structured fields (labels, priority) are validated against known enums before application.

2. **GitHub API trust**: Aptu trusts the GitHub API to return accurate issue and PR data. A compromised GitHub API session or account could cause incorrect data to be triaged. Mitigation: GitHub OAuth tokens are stored in the OS keyring and never logged.

3. **Solo maintainer availability**: Security patches depend on a single maintainer. If the maintainer is unavailable, patch response time may exceed the SLAs in SECURITY.md. Mitigation: Apache-2.0 license allows any contributor to fork and issue patches independently; see GOVERNANCE.md.

4. **Prompt injection via repository content**: Malicious issue or PR content may manipulate AI output to produce incorrect labels or misleading summaries. Aptu's `--dry-run` and `--no-apply` flags allow review before any changes are posted. There is no technical control that fully prevents AI manipulation by crafted input.

## References

- [SECURITY.md](../SECURITY.md) - Vulnerability disclosure process and response SLAs
- [GOVERNANCE.md](../GOVERNANCE.md) - Maintainer roles and succession plan
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) - System architecture and data flow
