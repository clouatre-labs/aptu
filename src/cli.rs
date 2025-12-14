//! Command-line interface definition for Aptu.
//!
//! Uses clap's derive API for declarative CLI parsing.

use std::io::IsTerminal;

use clap::{Parser, Subcommand, ValueEnum};

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
    /// Authenticate with GitHub via OAuth device flow
    Auth {
        /// Log out and remove stored credentials
        #[arg(long)]
        logout: bool,
    },

    /// List curated repositories available for contribution
    Repos,

    /// List open issues suitable for contribution
    Issues {
        /// Repository to filter issues (e.g., "block/goose")
        #[arg(short, long)]
        repo: Option<String>,
    },

    /// Triage an issue with AI assistance
    Triage {
        /// GitHub issue URL to triage
        issue_url: String,

        /// Preview triage without posting to GitHub
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt (post immediately)
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Show your contribution history
    History,
}
