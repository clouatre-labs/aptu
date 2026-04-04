# Repository Standards

Living reference mapping every CI artifact, workflow, and tooling choice to its purpose and rationale. Accurate against the current repository state; aspirational items are marked **TODO**.

---

## Workflow Artifact Map

| File | Trigger | Purpose | Rationale |
|------|---------|---------|-----------|
| `.github/workflows/ci.yml` | push/PR (src, tests, workflows) | Build, test, lint, security scan | Fast feedback on every change; path filters skip docs-only pushes |
| `.github/workflows/release.yml` | push `v*.*.*` tag, workflow_dispatch | Build release binaries, attest provenance, publish to GitHub Releases, Homebrew, Snap | Single pipeline owns the full release lifecycle |
| `.github/workflows/ios-build.yml` | push/PR (AptuApp, Cargo.toml, src) | Build and test iOS Swift app and Rust FFI bindings | Catches UniFFI binding regressions before merge |
| `.github/workflows/build-and-attest.yml` | push/PR | Build release binaries and attest provenance | SLSA Level 3 provenance attestation for every build |
| `.github/workflows/reuse.yml` | push/PR | REUSE SPDX compliance check | Apache-2.0 license attribution is machine-verifiable |
| `.github/workflows/scorecard.yml` | schedule weekly, push main | OpenSSF Scorecard security posture analysis | Tracks supply-chain security best practices over time |
| `.github/workflows/codeql.yml` | push/PR, schedule | CodeQL static analysis for security vulnerabilities | Automated SAST; catches common vulnerability patterns |
| `.github/workflows/issue-triage.yml` | issue opened/labeled | Auto-triage and label new issues | Reduces maintainer triage overhead |
| `.github/workflows/pr-triage.yml` | PR opened/labeled | Auto-label and route PRs by changed paths | Reduces maintainer triage overhead |

---

## Required Status Checks

The `ci-result` job in `ci.yml` aggregates all matrix and lint jobs. It is the sole required status check in the branch ruleset. All other individual jobs are gated through it.

---

## Cargo Profiles

| Profile | `opt-level` | `lto` | `codegen-units` | `panic` | `strip` | Purpose |
|---------|------------|-------|-----------------|---------|---------|---------|
| `release` | `z` (size) | `true` (full) | `1` | `abort` | `true` | Production binary; smallest size, deterministic |
| `ci` | inherits | `false` | `16` | inherits | inherits | CI builds; faster link time without sacrificing correctness |

`panic = "abort"` in release is intentional: no unwinding overhead. Do not pass `--profile ci` to `cargo test`; `panic=abort` aborts the test harness.

---

## Tooling

| Tool | Command | Purpose |
|------|---------|---------|
| `cargo clippy` | `cargo clippy --profile ci -- -D warnings` | Lint; all warnings are errors in CI |
| `cargo fmt` | `cargo fmt --check` | Format enforcement |
| `cargo deny` | `cargo deny check advisories licenses` | Dependency audit (CVEs and license policy) |
| `actionlint` | `actionlint` | GitHub Actions workflow syntax validation |
| `zizmor` | `zizmor .github/workflows/` | SHA pinning and security pattern enforcement for Actions |
| `gitleaks` | `gitleaks detect` | Secret detection in source history |
| `reuse` | `reuse lint` | SPDX header compliance |

### Lint suppressions

**Cognitive complexity threshold.** `clippy::cognitive_complexity` is enforced at a threshold of 30 (set in `clippy.toml`); `-D warnings` promotes violations to hard errors in CI. When a function legitimately exceeds the threshold and splitting it would reduce clarity rather than improve it, suppress with an attribute and a mandatory `reason` field:

```rust
#[allow(clippy::cognitive_complexity, reason = "<why this function cannot be meaningfully split>")]
```

Do not raise the global threshold to accommodate a single outlier. The `reason` field is required: it documents intent for reviewers and makes the suppression searchable. Macro-expanded code can inflate scores artificially; this is a known upstream limitation (rust-lang/rust-clippy#14417).

---

## Security Controls

| Control | Implementation | Rationale |
|---------|---------------|-----------|
| SLSA Level 3 provenance | `attest-build-provenance` + cosign in reusable `build-and-attest.yml` (`workflow_call`) | Verifiable artifact origin; reusable workflow isolation satisfies SLSA v1.0 Build Level 3; mitigates supply-chain substitution |
| OIDC keyless signing | `id-token: write` per-job in `release.yml` | No long-lived credentials; tokens scoped to the run |
| GPG tag signing | `git tag -s`; verified by `git verify-tag` with imported public key in `release.yml` | Guards against tag tampering before any build or publish runs |
| SHA-pinned Actions | All `uses:` lines pinned to commit SHA | Prevents tag mutation attacks (e.g., `actions/checkout@v4` is mutable) |
| gitleaks secret scan | Required check in `ci.yml` | Catches accidental credential commits |
| zizmor | Required check in `ci.yml` (path-filtered to `workflows/**`) | Enforces SHA pinning and flags unsafe workflow patterns |
| REUSE compliance | SPDX headers on every source file; checked in `reuse.yml` | Apache-2.0 license attribution is machine-verifiable |
| Least-privilege permissions | Top-level `permissions: contents: read` in every workflow; elevated scopes declared per-job | Limits blast radius if a step is compromised |
| OpenSSF Scorecard | `scorecard.yml` on schedule and push to main | Tracks supply-chain security best practices over time |
| cargo-deny | `cargo deny check advisories licenses` | Audit Rust dependency tree for CVEs and license policy |

**TODO:** Add [poutine](https://github.com/laurentsimon/poutine) for GitHub Actions supply-chain analysis (injection, unpinned actions in called workflows).

---

## Dependency Management

- **Renovate** manages all dependency updates automatically (config: `.github/renovate.json`).
- GitHub Actions digests are updated via `matchManagers: ["github-actions"]` with automerge enabled for digest/pin/patch/minor updates.
- Rust crates are updated via `matchManagers: ["cargo"]`.
- `minimumReleaseAge: 3 days` prevents merging dependencies the same day they are published (typosquatting window).
- `cargo deny check advisories licenses` runs in CI to audit the resolved dependency tree against known CVEs and the project license policy.

---

## Versioning and Release Conventions

- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `chore:`, `docs:`, `ci:`, etc.).
- PR titles must match the Conventional Commits format (enforced by `pr-triage.yml`).
- Release tags use the format `vMAJOR.MINOR.PATCH` and must be GPG-signed annotated tags (`git tag -s`).
- Version numbers are bumped in `Cargo.toml` workspace manifests and committed before the tag is pushed.
- GitHub Releases are created by `release.yml` automatically on tag push.
- CHANGELOG is generated from conventional commits.

---

## MCP Server Policy

The `aptu-mcp` binary ships with a `--read-only` flag. When enabled:

- Write tools (`post_triage`, `post_review`) are disabled and return an error.
- All read tools (`triage_issue`, `review_pr`, `scan_security`, `health`) remain available.

The server defaults to write-enabled. Pass `--read-only` to restrict it to read-only mode, disabling write operations in third-party integrations (e.g., Goose MCP extension) where unintended writes are a concern.

---

## Applying to a New Repository

1. Copy `.github/workflows/` and `.github/renovate.json`.
2. Create required GitHub secrets: `GPG_SIGNING_KEY` (ASCII-armored public key of the release signer).
3. Set branch ruleset: squash merge only, `delete_branch_on_merge: true`, required status check: `ci-result`.
4. Enable OIDC for the release environment in GitHub Settings > Environments.
5. Add SPDX headers to all source files (`reuse addheader`).
6. **TODO:** Configure poutine once adopted.
