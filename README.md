# Aptu

[![CI](https://github.com/clouatre-labs/aptu/actions/workflows/ci.yml/badge.svg)](https://github.com/clouatre-labs/aptu/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)

**AI-Powered Triage Utility** - A gamified CLI for OSS issue triage with AI assistance.

> *Aptu* (Mi'kmaq): "Paddle" - Navigate forward through open source contribution

## Features

- **GitHub OAuth** - Secure device flow authentication (or use existing `gh` CLI auth)
- **Issue Discovery** - Find "good first issue" from curated repositories
- **AI Triage** - Get summaries, suggested labels, clarifying questions, and contributor guidance via OpenRouter
- **Flexible Issue References** - Triage by URL, short form (owner/repo#123), or bare number
- **Already-Triaged Detection** - Automatically detects if you've already triaged an issue
- **Triage Flags** - Control behavior with `--show-issue`, `--force`, `--dry-run`, `--yes`
- **Multiple Output Formats** - Text, JSON, YAML, and Markdown output
- **Local History** - Track your contributions offline

## Installation

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

# Generate shell completions
aptu completion zsh > ~/.zsh/completions/_aptu
```

## Shell Completions

Enable tab completion for your shell:

**Bash** - Add to `~/.bashrc` or `~/.bash_profile`:
```bash
eval "$(aptu completion bash)"
```

**Zsh** - Generate completion file:
```zsh
mkdir -p ~/.zsh/completions
aptu completion zsh > ~/.zsh/completions/_aptu
```

Add to `~/.zshrc` (before compinit):
```zsh
fpath=(~/.zsh/completions $fpath)
autoload -U compinit && compinit -i
```

**Fish** - Generate completion file:
```fish
aptu completion fish > ~/.config/fish/completions/aptu.fish
```

**PowerShell** - Add to `$PROFILE`:
```powershell
aptu completion powershell | Out-String | Invoke-Expression
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

## Development

```bash
cargo test       # Run tests
cargo fmt        # Format code
cargo clippy     # Lint
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute, including our Developer Certificate of Origin (DCO) requirement.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

## Links

- [Full Specification](SPEC.md)
- [aptu.dev](https://aptu.dev) | [aptu.app](https://aptu.app)
