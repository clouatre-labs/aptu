<p align="center">
  <img src="https://raw.githubusercontent.com/clouatre-labs/aptu/main/assets/logo-light.png" alt="Aptu Logo" width="128">
</p>

<h1 align="center">Aptu</h1>

<p align="center">
  <a href="https://github.com/clouatre-labs/aptu"><img alt="github" src="https://img.shields.io/badge/github-clouatre--labs/aptu-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20"></a>
  <a href="https://crates.io/crates/aptu"><img alt="crates.io" src="https://img.shields.io/crates/v/aptu.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20"></a>
  <a href="https://docs.rs/aptu"><img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-aptu-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20"></a>
  <a href="https://github.com/clouatre-labs/aptu/actions?query=branch%3Amain"><img alt="build status" src="https://img.shields.io/github/actions/workflow/status/clouatre-labs/aptu/ci.yml?branch=main&style=for-the-badge" height="20"></a>
</p>

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img alt="license" src="https://img.shields.io/badge/license-Apache%202.0-blue?style=for-the-badge" height="20"></a>
  <a href="https://api.reuse.software/info/github.com/clouatre-labs/aptu"><img alt="REUSE" src="https://img.shields.io/badge/REUSE-compliant-green?style=for-the-badge" height="20"></a>
  <a href="https://www.rust-lang.org/"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.92.0-orange?style=for-the-badge" height="20"></a>
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
brew tap clouatre-labs/tap
brew install aptu
```

Or install via cargo-binstall (recommended, ~5 seconds):

```bash
cargo binstall aptu
```

Or compile from crates.io (~2-3 minutes):

```bash
cargo install aptu
```

Or build from source:

```bash
git clone https://github.com/clouatre-labs/aptu.git
cd aptu
cargo build --release
```

## Quick Start

```bash
# Authenticate with GitHub
aptu auth login

# Check authentication status
aptu auth status

# List curated repositories
aptu repo list

# Browse issues in a repo
aptu issue list block/goose

# Triage an issue with AI assistance
aptu issue triage https://github.com/block/goose/issues/123

# Preview triage without posting
aptu issue triage https://github.com/block/goose/issues/123 --dry-run

# View your contribution history
aptu history

# Install shell completions (auto-detects your shell)
aptu completion install
```

## GitHub Action

Automatically triage new issues in your repository using the Aptu GitHub Action. The action runs when issues are opened and posts AI-powered analysis and suggestions.

### Setup

1. Create a workflow file in your repository (`.github/workflows/triage.yml`):

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

      - name: Run Aptu Triage
        uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

2. Add your OpenRouter API key as a repository secret (`OPENROUTER_API_KEY`)

### Inputs

- **github-token** (required) - GitHub token for API access (use `secrets.GITHUB_TOKEN`)
- **openrouter-api-key** (required) - OpenRouter API key for AI analysis
- **model** (optional) - OpenRouter model to use (default: `mistralai/devstral-2512:free`)
- **skip-labeled** (optional) - Skip triage if issue already has labels (default: `true`)
- **dry-run** (optional) - Run without posting comments (default: `false`)
- **apply-labels** (optional) - Apply AI-suggested labels and milestone (default: `true`)

## Shell Completions

Enable tab completion for your shell:

```bash
aptu completion install
```

See [Shell Completions](docs/CONFIGURATION.md) for manual setup instructions.

## Configuration

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for detailed AI provider setup and configuration options.

## Supply Chain Security

Aptu follows supply chain security best practices:

- **GitHub Attestations** - Release artifacts are signed with [artifact attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-to-establish-provenance-for-builds) for build provenance
- **REUSE Compliant** - All files have machine-readable license metadata ([REUSE 3.3](https://reuse.software/))
- **Signed Commits** - All commits are GPG-signed with DCO sign-off
- **Optimized Binaries** - Release builds use LTO and size optimizations (~3MB binary)

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md) for guidelines on how to contribute, including our Developer Certificate of Origin (DCO) requirement. Whether you're interested in adding new AI models, building the iOS app, enhancing the CLI, writing documentation, reporting bugs, or spreading the word - all contributions are welcome!

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE) for details.
