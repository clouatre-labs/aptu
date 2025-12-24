<p align="center">
  <img src="https://raw.githubusercontent.com/clouatre-labs/aptu/main/assets/logo-light.png" alt="Aptu Logo" width="128">
</p>

<h1 align="center">Aptu</h1>

<p align="center">
  <a href="https://github.com/clouatre-labs/aptu"><img alt="github" src="https://img.shields.io/badge/github-clouatre--labs/aptu-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20"></a>
  <a href="https://crates.io/crates/aptu-cli"><img alt="crates.io" src="https://img.shields.io/crates/v/aptu-cli.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20"></a>
  <a href="https://docs.rs/aptu-cli"><img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-aptu--cli-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20"></a>
  <a href="https://github.com/clouatre-labs/aptu/actions?query=branch%3Amain"><img alt="build status" src="https://img.shields.io/github/actions/workflow/status/clouatre-labs/aptu/ci.yml?branch=main&style=for-the-badge" height="20"></a>
</p>

<p align="center"><strong>AI-Powered Triage Utility</strong> - A CLI for OSS issue triage with AI assistance.</p>

> *Aptu* (Mi'kmaq): "Paddle" - Navigate forward through open source contribution

## Demo

![Aptu Demo](https://raw.githubusercontent.com/clouatre-labs/aptu/main/assets/demo.gif)

## Features

- **GitHub OAuth** - Secure device flow authentication (or use existing `gh` CLI auth)
- **Issue Discovery** - Find "good first issue" from curated repositories
- **AI Triage** - Get summaries, suggested labels, clarifying questions, and contributor guidance via OpenRouter
- **Flexible Issue References** - Triage by URL, short form (owner/repo#123), or bare number
- **Already-Triaged Detection** - Automatically detects if you've already triaged an issue
- **Triage Flags** - Control behavior with `--dry-run`, `--yes`, `--since`
- **Multiple Output Formats** - Text, JSON, YAML, and Markdown output
- **Local History** - Track your contributions offline

## Installation

```bash
# Homebrew (recommended)
brew tap clouatre-labs/tap
brew install aptu

# Or via cargo-binstall (fast, ~5 seconds)
cargo binstall aptu

# Or from crates.io (~2-3 minutes)
cargo install aptu

# Or build from source
git clone https://github.com/clouatre-labs/aptu.git
cd aptu && cargo build --release
```

## Quick Start

```bash
aptu auth login                                                    # Authenticate with GitHub
aptu repo list                                                     # List curated repositories
aptu issue list block/goose                                        # Browse issues
aptu issue triage https://github.com/block/goose/issues/123       # Triage with AI
aptu issue triage https://github.com/block/goose/issues/123 --dry-run  # Preview
aptu history                                                       # View your contributions
```

## GitHub Action

Automatically triage new issues using the Aptu GitHub Action. Create `.github/workflows/triage.yml`:

```yaml
name: Triage New Issues
on:
  issues:
    types: [opened]
jobs:
  triage:
    runs-on: ubuntu-latest
    permissions:
      issues: write
      contents: read
    steps:
      - uses: actions/checkout@8e8c483db84b4bee98b60c0593521ed34d9990e8 # v6
      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

See [docs/GITHUB_ACTION.md](docs/GITHUB_ACTION.md) for detailed inputs documentation.

## Configuration

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for detailed AI provider setup and configuration options.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md) for guidelines on how to contribute, including our Developer Certificate of Origin (DCO) requirement. Whether you're interested in adding new AI models, building the iOS app, enhancing the CLI, writing documentation, reporting bugs, or spreading the word - all contributions are welcome!

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE) for details.
