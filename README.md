<p align="center">
  <img src="https://raw.githubusercontent.com/clouatre-labs/aptu/main/assets/logo-light.png" alt="Aptu Logo" width="128">
</p>

<h1 align="center">Aptu</h1>

<p align="center">
  <a href="https://crates.io/crates/aptu-cli"><img alt="crates.io" src="https://img.shields.io/crates/v/aptu-cli.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20"></a>
  <a href="https://docs.rs/aptu-core"><img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-aptu--core-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20"></a>
  <a href="https://github.com/clouatre-labs/aptu/actions?query=branch%3Amain"><img alt="build status" src="https://img.shields.io/github/actions/workflow/status/clouatre-labs/aptu/ci.yml?branch=main&style=for-the-badge" height="20"></a>
  <a href="https://api.reuse.software/info/github.com/clouatre-labs/aptu"><img alt="REUSE" src="https://api.reuse.software/badge/github.com/clouatre-labs/aptu" height="20"></a>
  <a href="https://slsa.dev"><img alt="SLSA Level 3" src="https://slsa.dev/images/gh-badge-level3.svg" height="20"></a>
  <a href="https://www.bestpractices.dev/projects/11662"><img alt="OpenSSF Best Practices" src="https://www.bestpractices.dev/projects/11662/badge" height="20"></a>
  <a href="https://scorecard.dev/viewer?uri=github.com/clouatre-labs/aptu"><img alt="OpenSSF Scorecard" src="https://api.scorecard.dev/projects/github.com/clouatre-labs/aptu/badge" height="20"></a>
</p>

<p align="center"><strong>AI-Powered Triage Utility</strong> - A CLI for OSS issue triage with AI assistance.</p>

Aptu is a context-engineering experiment: instead of throwing big models at problems, it crafts tight prompts that let smaller models (Devstral, Llama 3.3, Qwen) do the job with fewer tokens and surprising precision.

## Demo

![Aptu Demo](https://raw.githubusercontent.com/clouatre-labs/aptu/main/assets/demo.gif)

## Features

- **AI Triage** - Summaries, suggested labels, clarifying questions, and contributor guidance
- **Issue Discovery** - Find good-first-issues from curated repositories
- **PR Analysis** - AI-powered pull request review and feedback
- **GitHub Action** - Auto-triage incoming issues with labels and comments
- **Multiple Providers** - Gemini (default), Cerebras, Groq, OpenRouter, Z.AI, and ZenMux
- **Local History** - Track your contributions offline
- **Multiple Outputs** - Text, JSON, YAML, and Markdown

## Installation

```bash
# Homebrew (macOS/Linux)
brew install clouatre-labs/tap/aptu

# Snap (Linux)
snap install aptu

# Cargo-binstall (fast)
cargo binstall aptu-cli

# Cargo
cargo install aptu-cli
```

## Quick Start

```bash
aptu auth login                                                    # Authenticate with GitHub
aptu repo list                                                     # List curated repositories
aptu issue list block/goose                                        # Browse issues
aptu issue triage block/goose#123                                  # Triage with AI
aptu issue triage block/goose#123 --dry-run                        # Preview
aptu history                                                       # View your contributions
```

## GitHub Action

Auto-triage new issues with AI using any supported provider.

```yaml
- uses: clouatre-labs/aptu@v0
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    gemini-api-key: ${{ secrets.GEMINI_API_KEY }}
```

Options: `apply-labels`, `no-comment`, `skip-labeled`, `dry-run`, `model`, `provider`.

See [docs/GITHUB_ACTION.md](docs/GITHUB_ACTION.md) for setup and examples.

## Configuration

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for AI provider setup.

## Security

- **SLSA Level 3** - Provenance attestations for all releases
- **REUSE/SPDX** - License compliance for all files
- **Signed Commits** - GPG-signed commits required
- **Dependency Scanning** - Automated updates via Renovate

See [SECURITY.md](SECURITY.md) for reporting and verification.

## Contributing

We welcome contributions! See [CONTRIBUTING.md](https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md) for guidelines.

## License

Apache-2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE).
