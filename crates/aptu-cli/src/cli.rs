// SPDX-License-Identifier: Apache-2.0

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

/// Issue state filter for triage operations.
#[derive(Clone, Copy, Default, ValueEnum)]
pub enum IssueState {
    /// Only open issues (default)
    #[default]
    Open,
    /// Only closed issues
    Closed,
    /// Both open and closed issues
    All,
}

/// Global output configuration passed to commands.
#[derive(Clone)]
pub struct OutputContext {
    /// Output format (text, json, yaml)
    pub format: OutputFormat,
    /// Suppress non-essential output (spinners, progress)
    pub quiet: bool,
    /// Verbosity level: 0 = default, 1 = verbose (-v), 2+ = debug (-vv)
    pub verbosity: u8,
    /// Whether stdout is a terminal (TTY)
    pub is_tty: bool,
}

impl OutputContext {
    /// Creates an `OutputContext` from CLI arguments.
    pub fn from_cli(format: OutputFormat, quiet: bool, verbosity: u8) -> Self {
        Self {
            format,
            quiet,
            verbosity,
            is_tty: std::io::stdout().is_terminal(),
        }
    }

    /// Returns true if interactive elements (spinners, colors) should be shown.
    pub fn is_interactive(&self) -> bool {
        self.is_tty && !self.quiet && matches!(self.format, OutputFormat::Text)
    }

    /// Returns true if verbose output is enabled (-v or higher).
    pub fn is_verbose(&self) -> bool {
        self.verbosity >= 1
    }

    /// Returns true if debug output is enabled (-vv or higher).
    #[allow(dead_code)]
    pub fn is_debug(&self) -> bool {
        self.verbosity >= 2
    }
}

/// Parses a date string in YYYY-MM-DD or RFC3339 format and returns RFC3339 string.
///
/// Converts YYYY-MM-DD to RFC3339 format (midnight UTC) for GraphQL filtering.
///
/// # Arguments
///
/// * `date_str` - Date string in YYYY-MM-DD or RFC3339 format
///
/// # Returns
///
/// RFC3339 formatted date string, or the input if already in RFC3339 format
///
/// # Errors
///
/// Returns an error if the date format is invalid.
pub fn parse_date_to_rfc3339(date_str: &str) -> anyhow::Result<String> {
    // Try RFC3339 format first
    if chrono::DateTime::parse_from_rfc3339(date_str).is_ok() {
        return Ok(date_str.to_string());
    }

    // Try YYYY-MM-DD format
    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        let datetime = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow::anyhow!("Failed to create datetime from date {date_str}"))?;
        let rfc3339 = format!("{}Z", datetime.format("%Y-%m-%dT%H:%M:%S"));
        return Ok(rfc3339);
    }

    anyhow::bail!("Invalid date format. Expected YYYY-MM-DD or RFC3339 format, got: {date_str}")
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

    /// Enable verbose output (debug-level logging). Use -v for verbose, -vv for debug
    #[arg(long, short = 'v', global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Override configured AI provider (e.g., openrouter, anthropic)
    #[arg(long, global = true)]
    pub provider: Option<String>,

    /// Override configured AI model (e.g., gpt-4, claude-3-opus)
    #[arg(long, global = true)]
    pub model: Option<String>,

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

    /// Work with pull requests
    #[command(subcommand)]
    Pr(PrCommand),

    /// Generate AI-curated release notes from PRs between tags
    Release {
        /// Tag to generate release notes for (defaults to inferring previous tag)
        tag: Option<String>,

        /// Repository in owner/repo format (inferred from git if not provided)
        #[arg(long)]
        repo: Option<String>,

        /// Starting tag (defaults to previous tag)
        #[arg(long)]
        from: Option<String>,

        /// Ending tag (defaults to HEAD)
        #[arg(long)]
        to: Option<String>,

        /// Generate release notes for unreleased changes (HEAD since last tag)
        #[arg(long)]
        unreleased: bool,

        /// Post release notes to GitHub
        #[arg(long)]
        update: bool,

        /// Preview release notes without posting
        #[arg(long)]
        dry_run: bool,
    },

    /// Show your contribution history
    History,

    /// List AI models from providers
    #[command(subcommand)]
    Models(ModelsCommand),

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
    /// List repositories available for contribution
    List {
        /// Include only curated repositories
        #[arg(long)]
        curated: bool,

        /// Include only custom repositories
        #[arg(long)]
        custom: bool,
    },

    /// Discover welcoming repositories on GitHub
    Discover {
        /// Programming language to filter by (e.g., Rust, Python)
        #[arg(long)]
        language: Option<String>,

        /// Minimum number of stars
        #[arg(long, default_value = "10")]
        min_stars: u32,

        /// Maximum number of results to return
        #[arg(long, default_value = "20")]
        limit: u32,
    },

    /// Add a custom repository
    Add {
        /// Repository in owner/name format
        repo: String,
    },

    /// Remove a custom repository
    Remove {
        /// Repository in owner/name format
        repo: String,
    },
}

/// Issue subcommands
#[derive(Subcommand)]
pub enum IssueCommand {
    /// List open issues suitable for contribution
    List {
        /// Repository to filter issues (e.g., "block/goose")
        repo: Option<String>,

        /// Disable caching of issue data
        #[arg(long)]
        no_cache: bool,
    },

    /// Triage an issue with AI assistance
    Triage {
        /// Issue references (URL, owner/repo#number, or number)
        #[arg(value_name = "REFERENCE")]
        references: Vec<String>,

        /// Repository for bare issue numbers (e.g., "block/goose")
        #[arg(long, short = 'r')]
        repo: Option<String>,

        /// Triage all issues without labels created since this date (YYYY-MM-DD or RFC3339 format)
        #[arg(long)]
        since: Option<String>,

        /// Filter issues by state when using --since (open, closed, or all)
        #[arg(long, short = 's', default_value = "open")]
        state: IssueState,

        /// Preview triage without posting to GitHub
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt (post immediately)
        #[arg(short = 'y', long)]
        yes: bool,

        /// Apply AI-suggested labels and milestone to the issue (additive: merges with existing labels, preserves existing priority labels and milestone)
        #[arg(long)]
        apply: bool,

        /// Skip posting triage comment to GitHub
        #[arg(long)]
        no_comment: bool,

        /// Bypass 'already triaged' detection
        #[arg(short, long)]
        force: bool,
    },

    /// Create a GitHub issue with AI assistance
    Create {
        /// Repository for the issue (e.g., "owner/repo")
        repo: String,

        /// Issue title (interactive prompt if not provided)
        #[arg(long)]
        title: Option<String>,

        /// Issue body/description (interactive prompt if not provided)
        #[arg(long)]
        body: Option<String>,

        /// Read issue content from file (text or markdown)
        #[arg(long)]
        from: Option<String>,

        /// Preview issue creation without posting to GitHub
        #[arg(long)]
        dry_run: bool,
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

/// Pull request subcommands
#[derive(Subcommand)]
pub enum PrCommand {
    /// Review a pull request with AI assistance
    Review {
        /// PR reference (URL, owner/repo#number, or number)
        #[arg(value_name = "REFERENCE")]
        reference: String,

        /// Repository for bare PR numbers (e.g., "block/goose")
        #[arg(long, short = 'r')]
        repo: Option<String>,

        /// Post review as a comment (read-only, no approval)
        #[arg(long, group = "review_type")]
        comment: bool,

        /// Post review with approval
        #[arg(long, group = "review_type")]
        approve: bool,

        /// Post review requesting changes
        #[arg(long, group = "review_type")]
        request_changes: bool,

        /// Preview the review without posting
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt when posting
        #[arg(long)]
        yes: bool,
    },
    /// Auto-label a pull request based on conventional commit prefix and file paths
    Label {
        /// PR reference (URL, owner/repo#number, or number)
        #[arg(value_name = "REFERENCE")]
        reference: String,

        /// Repository for bare PR numbers (e.g., "block/goose")
        #[arg(long, short = 'r')]
        repo: Option<String>,

        /// Preview labels without applying
        #[arg(long)]
        dry_run: bool,
    },
}

/// AI models subcommands
#[derive(Subcommand)]
pub enum ModelsCommand {
    /// List available AI models from a provider
    List {
        /// AI provider name (e.g., "openrouter", "openai")
        #[arg(long)]
        provider: String,

        /// Show only free models
        #[arg(long)]
        free: bool,

        /// Force cache refresh (ignore 24h TTL)
        #[arg(long)]
        refresh: bool,
    },
}
