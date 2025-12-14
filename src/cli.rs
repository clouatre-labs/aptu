//! Command-line interface definition for Aptu.
//!
//! Uses clap's derive API for declarative CLI parsing.

use clap::{Parser, Subcommand};

/// Aptu - Gamified OSS issue triage with AI assistance.
///
/// A CLI tool that helps developers contribute meaningfully to open source
/// projects through AI-assisted issue triage and PR review.
#[derive(Parser)]
#[command(name = "aptu")]
#[command(version, about, long_about = None)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Authenticate with GitHub via OAuth device flow
    Auth,

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
    },

    /// Show your contribution history
    History,
}
