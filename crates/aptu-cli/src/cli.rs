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
    /// SARIF output for security scanning tools
    Sarif,
    /// GitHub Actions annotation output (`::error file=...,line=...,title=...::message`)
    #[value(name = "github-annotations")]
    GithubAnnotations,
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
    /// Output format (json, github-annotations, sarif)
    pub format: OutputFormat,
    /// Suppress non-essential output (spinners, progress)
    pub quiet: bool,
    /// Verbose output enabled (-v flag)
    pub verbose: bool,
    /// Whether stdout is a terminal (TTY)
    pub is_tty: bool,
}

impl OutputContext {
    /// Creates an `OutputContext` from CLI arguments.
    /// Quiet mode is automatically enabled for structured formats (Json, Sarif).
    pub fn from_cli(format: OutputFormat, verbose: bool) -> Self {
        let quiet = matches!(
            format,
            OutputFormat::Json | OutputFormat::Sarif | OutputFormat::GithubAnnotations
        );
        Self {
            format,
            quiet,
            verbose,
            is_tty: std::io::stdout().is_terminal(),
        }
    }

    /// Returns true if interactive elements (spinners, colors) should be shown.
    pub fn is_interactive(&self) -> bool {
        self.is_tty && !self.quiet && matches!(self.format, OutputFormat::Text)
    }

    /// Returns true if verbose output is enabled (-v flag).
    pub fn is_verbose(&self) -> bool {
        self.verbose
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
    /// Output format (text, json, sarif, github-annotations)
    #[arg(long, short = 'o', global = true, value_enum, default_value = "text")]
    pub output: OutputFormat,

    /// Enable verbose output
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    /// Override configured AI provider (e.g., openrouter, anthropic)
    #[arg(long, global = true)]
    pub provider: Option<String>,

    /// Override configured AI model (e.g., gpt-4, claude-sonnet-4-6)
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Repository inferred from git remote (set by main.rs, not user-facing)
    #[arg(skip)]
    pub inferred_repo: Option<String>,

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

    /// Work with GitHub issues
    #[command(subcommand)]
    Issue(IssueCommand),

    /// Work with pull requests
    #[command(subcommand)]
    Pr(PrCommand),

    /// Generate or install shell completion scripts
    #[command(subcommand)]
    Completion(CompletionCommand),

    /// Scan a file or directory for security issues
    ScanSecurity {
        /// Path to scan (file or directory)
        #[arg(required_unless_present = "diff")]
        path: Option<std::path::PathBuf>,
        /// Read unified diff from FILE or - for stdin
        #[arg(long, conflicts_with = "path", value_name = "FILE")]
        diff: Option<std::path::PathBuf>,
        /// Fail with exit code 1 if findings match these severities (comma-separated: critical,high,medium,low)
        #[arg(long, value_delimiter = ',')]
        fail_on: Vec<String>,
        /// Exclude paths matching this prefix (repeatable)
        #[arg(long)]
        exclude: Vec<String>,
        /// Write SARIF output to this file
        #[arg(long, value_name = "PATH")]
        sarif_output: Option<std::path::PathBuf>,
    },
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

/// Issue subcommands
#[derive(Subcommand)]
pub enum IssueCommand {
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

        /// Skip applying AI-suggested labels and milestone to the issue
        #[arg(long)]
        no_apply: bool,

        /// Skip posting triage comment to GitHub
        #[arg(long)]
        no_comment: bool,

        /// Bypass 'already triaged' detection
        #[arg(short, long)]
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

/// Pull request subcommands
#[derive(Subcommand)]
pub enum PrCommand {
    /// Review a pull request with AI assistance
    Review {
        /// PR references (URL, owner/repo#number, or number)
        #[arg(value_name = "REFERENCE")]
        references: Vec<String>,

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

        /// Skip applying labels and milestone to the PR
        #[arg(long)]
        no_apply: bool,

        /// Skip posting review comment to GitHub
        #[arg(long)]
        no_comment: bool,

        /// Disable review summary dedup (always post a new summary comment)
        #[arg(long)]
        no_dedup_summary: bool,

        /// Bypass confirmation prompts
        #[arg(short, long)]
        force: bool,

        /// Path to the local repository root for AST context injection. Optional when running from within the repo.
        #[arg(long, value_name = "PATH")]
        repo_path: Option<std::path::PathBuf>,

        /// Path to repository instructions file (overrides default AGENTS.md and .github/instructions/pr-review.md).
        #[arg(long, value_name = "PATH")]
        instructions_file: Option<std::path::PathBuf>,
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

    /// List and rank open pull requests by reviewability
    ///
    /// Fetches open PRs for a repository and ranks them by a composite score:
    /// 60% size (smaller = higher priority) + 40% age (older = higher priority).
    /// Draft PRs are excluded from the ranking (not ready for review).
    ///
    /// Default limit is 10; use --limit 0 to show all.
    Queue {
        /// Repository in owner/repo format (inferred from git if not provided)
        #[arg(long, short = 'r')]
        repo: Option<String>,

        /// Maximum number of PRs to display (0 = no limit)
        #[arg(long, default_value = "10")]
        limit: u32,
    },
}
