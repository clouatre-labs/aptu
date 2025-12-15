# Aptu

[![CI](https://github.com/clouatre-labs/aptu/actions/workflows/ci.yml/badge.svg)](https://github.com/clouatre-labs/aptu/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)

**AI-Powered Triage Utility** - A gamified CLI for OSS issue triage with AI assistance.

> *Aptu* (Mi'kmaq): "Paddle" - Navigate forward through open source contribution

## Features

- **GitHub OAuth** - Secure device flow authentication (or use existing `gh` CLI auth)
- **Issue Discovery** - Find "good first issue" from curated repositories
- **AI Triage** - Get summaries, suggested labels, and clarifying questions via OpenRouter
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

## License

MIT - See [LICENSE](LICENSE) for details.

## Links

- [Full Specification](SPEC.md)
- [aptu.dev](https://aptu.dev) | [aptu.app](https://aptu.app)
