<p align="center">
  <img src="https://raw.githubusercontent.com/clouatre-labs/aptu/main/assets/logo-light.png" alt="Aptu Logo" width="128">
</p>

<h1 align="center">Aptu</h1>

<p align="center">
  <a href="https://github.com/clouatre-labs/aptu/actions/workflows/ci.yml"><img src="https://github.com/clouatre-labs/aptu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/clouatre-labs/aptu/releases/latest"><img src="https://img.shields.io/github/v/release/clouatre-labs/aptu" alt="Release"></a>
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://api.reuse.software/info/github.com/clouatre-labs/aptu"><img src="https://api.reuse.software/badge/github.com/clouatre-labs/aptu" alt="REUSE Compliant"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/MSRV-1.92.0-orange.svg" alt="MSRV"></a>
  <a href="https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md"><img src="https://img.shields.io/badge/Contributors-Welcome-brightgreen.svg" alt="Contributors Welcome"></a>
</p>

<p align="center"><strong>AI-Powered Triage Utility</strong> - A gamified CLI for OSS issue triage with AI assistance.</p>

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

## Project Structure

Aptu is organized as a Rust workspace with multiple crates:

```
aptu/
├── aptu-cli/          # CLI binary (user-facing commands)
├── aptu-core/         # Shared library (GitHub API, AI, config)
├── aptu-ffi/          # FFI bindings for iOS (Phase 2+)
├── AptuApp/           # SwiftUI iOS app (Phase 2+)
└── tests/             # Integration tests (Bats)
```

**Key Crates:**
- **aptu-core** - Business logic, API clients, configuration management
- **aptu-cli** - Command-line interface and user interactions
- **aptu-ffi** - Rust-to-Swift bridge via UniFFI (iOS support)

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

Enable tab completion for your shell using the automated installer:

```bash
# Auto-detect shell and install
aptu completion install

# Preview without writing files
aptu completion install --dry-run

# Explicit shell selection
aptu completion install --shell zsh
```

The installer writes completions to standard locations and prints configuration instructions.

### Manual Setup

If you prefer manual setup, use `aptu completion generate <shell>`:

**Bash** - Add to `~/.bashrc` or `~/.bash_profile`:
```bash
eval "$(aptu completion generate bash)"
```

**Zsh** - Generate completion file:
```zsh
mkdir -p ~/.zsh/completions
aptu completion generate zsh > ~/.zsh/completions/_aptu
```

Add to `~/.zshrc` (before compinit):
```zsh
fpath=(~/.zsh/completions $fpath)
autoload -U compinit && compinit -i
```

**Fish** - Generate completion file:
```fish
aptu completion generate fish > ~/.config/fish/completions/aptu.fish
```

**PowerShell** - Add to `$PROFILE`:
```powershell
aptu completion generate powershell | Out-String | Invoke-Expression
```

Run `aptu completion --help` for more options.

## Configuration

Config file: `~/.config/aptu/config.toml`

```toml
[ai]
provider = "openrouter"
model = "mistralai/devstral-2512:free"

[ui]
confirm_before_post = true
```

Set your OpenRouter API key:

```bash
export OPENROUTER_API_KEY="sk-or-..."
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md) for guidelines on how to contribute, including our Developer Certificate of Origin (DCO) requirement.

## Supply Chain Security

Aptu follows supply chain security best practices:

- **GitHub Attestations** - Release artifacts are signed with [artifact attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-to-establish-provenance-for-builds) for build provenance
- **REUSE Compliant** - All files have machine-readable license metadata ([REUSE 3.3](https://reuse.software/))
- **Signed Commits** - All commits are GPG-signed with DCO sign-off
- **Optimized Binaries** - Release builds use LTO and size optimizations (~3MB binary)

## Contributors Welcome

We're actively seeking contributors to help expand Aptu! Whether you're interested in:

- Adding new AI models or improving triage quality
- Building the iOS app (Phase 2)
- Enhancing the CLI experience
- Writing documentation or tests
- Reporting bugs or suggesting features
- Spreading the word - blog posts, social media, talks

Please see [CONTRIBUTING.md](https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md) for guidelines and how to get started. All contributions are welcome!

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE) for details.

---

If Aptu helps your OSS workflow, consider giving it a star!
