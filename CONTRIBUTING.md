# Contributing to Aptu

We welcome contributions! This document covers the essentials.

## Non-Code Contributions

Not a coder? You can still help Aptu grow:

- **Write about Aptu** - Blog posts, tutorials, comparisons
- **Share on social media** - Twitter/X, Mastodon, LinkedIn, Reddit
- **Submit to newsletters** - This Week in Rust, Hacker News, dev.to
- **Give talks** - Meetups, conferences, podcasts
- **Create videos** - Demos, tutorials, reviews

## Quick Start

### Prerequisites

- **Rust 1.92.0** - Automatically managed via `rust-toolchain.toml`
- **Just** - Task runner for common commands

Install Just:
```bash
# macOS
brew install just

# Linux
cargo install just

# Or see https://github.com/casey/just#installation
```

### Setup and Development Commands

```bash
git clone https://github.com/YOUR_USERNAME/aptu.git
cd aptu

# List all available commands
just

# Run format, lint, and test (recommended before commits)
just check

# Individual commands
just fmt          # Check code formatting
just fmt-fix      # Auto-fix formatting
just lint         # Run clippy linter
just lint-fix     # Auto-fix clippy issues
just test         # Run unit tests
just integration  # Run integration tests
just build        # Build debug binary
just build-release # Build optimized release binary
just ci           # Run full CI pipeline locally
just reuse        # Check REUSE license compliance
just install      # Install binary to ~/.cargo/bin/
just clean        # Remove build artifacts
```

### Manual Commands (without Just)

If you prefer not to use Just:

```bash
cargo test       # Run tests
cargo fmt        # Format code
cargo clippy     # Lint
cargo build      # Build binary
```

## Before Submitting

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Fuzzing

Aptu includes cargo-fuzz targets to test parser robustness. Fuzzing requires Rust nightly:

```bash
# List available fuzz targets
cargo +nightly fuzz list

# Run the TOML parser fuzz target
cargo +nightly fuzz run parse_toml

# Run with a specific timeout (in seconds)
cargo +nightly fuzz run parse_toml -- -max_total_time=60
```

Fuzz targets are located in `fuzz/fuzz_targets/` and are independent from the main workspace.

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

## GitHub API Strategy

We use a hybrid GraphQL + REST approach via Octocrab. **Default to REST unless GraphQL provides a clear benefit.**

### Decision Heuristic

Ask: *Does GraphQL save enough API calls to justify custom query/struct overhead?*

**Use GraphQL when:**
- Fetching **3+ related resource types** in one call (e.g., issue + labels + milestones + comments)
- Batching **across multiple repos** using aliases
- **Server-side filtering** reduces payload significantly

**Use REST (Octocrab) when:**
- Fetching **1-2 resource types** (e.g., list issues, get single issue)
- Performing **mutations** (create, update, delete)
- **Client-side filtering** is required anyway (negates GraphQL's advantage)
- Octocrab has **typed builders** that match your use case

### Examples

| Scenario | Choice | Reasoning |
|----------|--------|-----------|
| Single-issue triage (issue + labels + milestones + assignees) | GraphQL | 1 call vs 4 REST calls |
| List untriaged issues (filter by empty labels) | REST | Single resource, client-side filter anyway |
| Update issue labels | REST | Mutation, Octocrab has `issues().update()` |
| Fetch issues from 5 repos | GraphQL | Aliases batch into 1 call vs 5 REST calls |

### File Locations

- `github/graphql.rs` - Custom GraphQL queries and response types
- `github/issues.rs` - REST operations via Octocrab typed builders

**Rate limits**: Both share the same pool (5000/hour authenticated).

## Branch Protection

The `main` branch is protected by GitHub rulesets with the following rules:

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
5. Edit the release to add highlights (see below)

The workflow builds binaries (macOS ARM64, Linux ARM64/x86_64), signs artifacts with cosign, generates SLSA attestations, creates a GitHub release with auto-generated notes, publishes to crates.io, and updates the Homebrew formula.

### Release Notes

We use a hybrid approach: GitHub auto-generates a changelog from conventional commits, and maintainers add a curated "Highlights" section for user-facing communication.

After the workflow completes, edit the release on GitHub to prepend:

```markdown
## [Theme or Summary]

Brief description of what this release delivers.

### Highlights

- **Feature Name** - One-line description
- **Another Feature** - One-line description

---

## Installation

**Homebrew (macOS/Linux)**
\`\`\`bash
brew install clouatre-labs/tap/aptu
\`\`\`

**Cargo**
\`\`\`bash
cargo install aptu-cli
\`\`\`

---

[Auto-generated changelog follows]
```

### Dry Run

Test the release workflow before tagging:

```bash
gh workflow run release.yml -f dry_run=true -f version=X.Y.Z
```

This builds all targets without publishing or creating a release.

### Versioning

We follow [SemVer](https://semver.org/): MAJOR (breaking), MINOR (features), PATCH (fixes).

## License

By contributing, you agree your contributions are licensed under [Apache-2.0](LICENSE).
