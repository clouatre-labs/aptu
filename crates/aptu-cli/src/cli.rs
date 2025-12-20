//! Command-line interface definition for Aptu.
//!
//! Uses clap's derive API for declarative CLI parsing with hierarchical
//! noun-verb subcommands for autocomplete-optimal design.

use std::io::IsTerminal;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

/// Extended help text for the generate subcommand with shell-specific examples.
const COMPLETION_GENERATE_HELP: &str = r#"EXAMPLES

  bash
    Add to ~/.bashrc or ~/.bash_profile:
      eval "$(aptu completion generate bash)"

  zsh
    Generate completion file:
      mkdir -p ~/.zsh/completions
      aptu completion generate zsh > ~/.zsh/completions/_aptu

    Add to ~/.zshrc (before compinit):
      fpath=(~/.zsh/completions $fpath)
      autoload -U compinit && compinit -i

  fish
    Generate completion file:
      aptu completion generate fish > ~/.config/fish/completions/aptu.fish

  PowerShell
    Add to $PROFILE:
      aptu completion generate powershell | Out-String | Invoke-Expression
"#;

/// Output format for CLI results.
#[derive(Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text with colors (default)
    #[default]
    Text,
    /// JSON output for programmatic consumption
    Json,
    /// YAML output for programmatic consumption
    Yaml,
    /// Markdown output for GitHub comments
    Markdown,
}

/// Global output configuration passed to commands.
pub struct OutputContext {
    /// Output format (text, json, yaml)
    pub format: OutputFormat,
    /// Suppress non-essential output (spinners, progress)
    pub quiet: bool,
    /// Whether stdout is a terminal (TTY)
    pub is_tty: bool,
}

impl OutputContext {
    /// Creates an `OutputContext` from CLI arguments.
    pub fn from_cli(format: OutputFormat, quiet: bool) -> Self {
        Self {
            format,
            quiet,
            is_tty: std::io::stdout().is_terminal(),
        }
    }

    /// Returns true if interactive elements (spinners, colors) should be shown.
    pub fn is_interactive(&self) -> bool {
        self.is_tty && !self.quiet && matches!(self.format, OutputFormat::Text)
    }
}

/// Aptu - Gamified OSS issue triage with AI assistance.
///
/// A CLI tool that helps developers contribute meaningfully to open source
/// projects through AI-assisted issue triage and PR review.
#[derive(Parser)]
#[command(name = "aptu")]
#[command(version, about, long_about = None)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Output format (text, json, yaml)
    #[arg(long, short = 'o', global = true, default_value = "text", value_enum)]
    pub output: OutputFormat,

    /// Suppress non-essential output (spinners, progress)
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Manage GitHub authentication
    #[command(subcommand)]
    Auth(AuthCommand),

    /// Manage curated repositories
    #[command(subcommand)]
    Repo(RepoCommand),

    /// Work with GitHub issues
    #[command(subcommand)]
    Issue(IssueCommand),

    /// Show your contribution history
    History,

    /// Generate or install shell completion scripts
    #[command(subcommand)]
    Completion(CompletionCommand),
}

/// Authentication subcommands
#[derive(Subcommand)]
pub enum AuthCommand {
    /// Authenticate with GitHub via OAuth device flow
    Login,

    /// Remove stored credentials
    Logout,

    /// Show current authentication status
    Status,
}

/// Repository subcommands
#[derive(Subcommand)]
pub enum RepoCommand {
    /// List curated repositories available for contribution
    List,
}

/// Issue subcommands
#[derive(Subcommand)]
pub enum IssueCommand {
    /// List open issues suitable for contribution
    List {
        /// Repository to filter issues (e.g., "block/goose")
        repo: Option<String>,
    },

    /// Triage an issue with AI assistance
    Triage {
        /// Issue reference (URL, owner/repo#number, or number)
        reference: String,

        /// Repository for bare issue numbers (e.g., "block/goose")
        #[arg(long, short = 'r')]
        repo: Option<String>,

        /// Preview triage without posting to GitHub
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt (post immediately)
        #[arg(short = 'y', long)]
        yes: bool,

        /// Display fetched issue content before AI triage analysis
        #[arg(long)]
        show_issue: bool,

        /// Force triage even if issue appears already triaged
        #[arg(long)]
        force: bool,
    },
}

/// Completion subcommands
#[derive(Subcommand)]
pub enum CompletionCommand {
    /// Generate completion script for a shell (output to stdout)
    #[command(after_long_help = COMPLETION_GENERATE_HELP)]
    Generate {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Install completion script to standard location
    Install {
        /// Shell to install completions for (auto-detected from $SHELL if not provided)
        #[arg(long, value_enum)]
        shell: Option<Shell>,

        /// Preview installation without writing files
        #[arg(long)]
        dry_run: bool,
    },
}
