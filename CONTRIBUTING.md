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

## License

By contributing, you agree your contributions are licensed under [Apache-2.0](LICENSE).
