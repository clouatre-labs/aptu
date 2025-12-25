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

Aptu is a context-engineering experiment: instead of throwing big models at problems, it crafts tight prompts that let smaller models (Devstral, Llama 3.3, Qwen) do the job with fewer tokens and surprising precision.

## Demo

![Aptu Demo](https://raw.githubusercontent.com/clouatre-labs/aptu/main/assets/demo.gif)

## Features

- **AI Triage** - Summaries, suggested labels, clarifying questions, and contributor guidance
- **Issue Discovery** - Find good-first-issues from curated repositories
- **PR Analysis** - AI-powered pull request review and feedback
- **GitHub Action** - Auto-triage incoming issues with labels and comments
- **Multiple Providers** - Gemini, OpenRouter, Groq, and Cerebras
- **Local History** - Track your contributions offline
- **Multiple Outputs** - Text, JSON, YAML, and Markdown

## Installation

```bash
# Homebrew (recommended)
brew tap clouatre-labs/tap
brew install aptu-cli

# Or via cargo-binstall (fast, ~5 seconds)
cargo binstall aptu-cli

# Or from crates.io (~2-3 minutes)
cargo install aptu-cli

# Or build from source
git clone https://github.com/clouatre-labs/aptu.git
cd aptu && cargo build --release
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

Auto-triage new issues. Create `.github/workflows/triage.yml`:

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
      - uses: actions/checkout@v6
      - uses: clouatre-labs/aptu@v0
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          openrouter-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          apply-labels: true  # Apply AI-suggested labels
```

See [docs/GITHUB_ACTION.md](docs/GITHUB_ACTION.md) for all options.

## Configuration

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for AI provider setup.

## Contributing

We welcome contributions! See [CONTRIBUTING.md](https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md) for guidelines.

## License

Apache-2.0. See [LICENSE](https://github.com/clouatre-labs/aptu/blob/main/LICENSE).
