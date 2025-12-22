# Contributing to Aptu

We welcome contributions! This document covers the essentials.

## Quick Start

```bash
git clone https://github.com/YOUR_USERNAME/aptu.git
cd aptu
cargo build && cargo test
```

## Before Submitting

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Commit Message Format

We follow [Conventional Commits](https://www.conventionalcommits.org/) to enable automated semantic versioning and changelog generation. All commits must follow this format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- **feat**: A new feature
- **fix**: A bug fix
- **docs**: Documentation only changes
- **style**: Changes that do not affect the meaning of the code (formatting, missing semicolons, etc.)
- **refactor**: A code change that neither fixes a bug nor adds a feature
- **perf**: A code change that improves performance
- **test**: Adding missing tests or correcting existing tests
- **chore**: Changes to build process, dependencies, or tooling

### Examples

```bash
# Feature with scope
git commit -s -m "feat(cli): add support for custom config paths"

# Bug fix
git commit -s -m "fix: resolve panic when parsing invalid labels"

# Breaking change
git commit -s -m "feat!: redesign API for issue filtering

BREAKING CHANGE: The --filter flag has been replaced with --query"

# Documentation
git commit -s -m "docs: update installation instructions"
```

### Breaking Changes

Mark breaking changes with `!` after the type/scope or use `BREAKING CHANGE:` in the footer:

```bash
git commit -s -m "feat!: change default behavior of triage command"
```

## Developer Certificate of Origin (DCO)

All commits must be signed off to certify you have the right to submit the code:

```bash
git commit -s -m "Your commit message"
```

This adds `Signed-off-by: Your Name <email>` to your commit, certifying you agree to the [DCO](https://developercertificate.org/).

## Pull Request Checklist

- [ ] Tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Commits signed off (`git commit -s`)
- [ ] Clear PR description

## Code Style

- Follow Rust idioms
- Use `cargo fmt`
- Address clippy warnings
- Write tests for new features

## Branch Protection

The `main` branch is protected by the following rules:

- **Required Status Checks**: All CI checks must pass before merging
  - `Check Labels`: Validates PR labels
  - `Lint`: Code formatting and linting checks
  - `Test`: All tests must pass
- **Signed Commits**: All commits must be signed (GPG or S/MIME)
- **No Force Push**: History cannot be rewritten on main
- **No Deletion**: The main branch cannot be deleted

These protections ensure code quality and maintain a clean history. Make sure your commits are signed and all CI checks pass before opening a pull request.

## Releasing

Releases are automated via GitHub Actions. Maintainers with push access:

1. Update version in `Cargo.toml`
2. Commit: `git commit -S -s -m "chore: bump version to X.Y.Z"`
3. Tag: `git tag -s vX.Y.Z -m "vX.Y.Z"`
4. Push: `git push origin main --tags`

The workflow builds binaries (macOS ARM64, Linux ARM64/x86_64), generates SLSA attestations, creates a GitHub release with auto-generated notes, publishes to crates.io, and updates the Homebrew formula.

We follow [SemVer](https://semver.org/): MAJOR (breaking), MINOR (features), PATCH (fixes).

## License

By contributing, you agree your contributions are licensed under [Apache-2.0](LICENSE).
