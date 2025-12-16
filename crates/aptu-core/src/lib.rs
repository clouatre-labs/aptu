//! # Aptu Core
//!
//! Core library for the Aptu CLI - AI-powered OSS issue triage.
//!
//! This crate provides reusable components for:
//! - GitHub API integration (authentication, issues, GraphQL)
//! - AI-assisted issue triage via `OpenRouter`
//! - Configuration management
//! - Contribution history tracking
//! - Curated repository discovery
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use aptu_core::{load_config, analyze_issue, IssueDetails};
//! use anyhow::Result;
//!
//! # async fn example() -> Result<()> {
//! // Load configuration
//! let config = load_config()?;
//!
//! // Create issue details
//! let issue = IssueDetails {
//!     owner: "block".to_string(),
//!     repo: "goose".to_string(),
//!     number: 123,
//!     title: "Example issue".to_string(),
//!     body: "Issue description...".to_string(),
//!     labels: vec![],
//!     comments: vec![],
//!     url: "https://github.com/block/goose/issues/123".to_string(),
//! };
//!
//! // Analyze with AI
//! let triage = analyze_issue(&config.ai, &issue).await?;
//! println!("Summary: {}", triage.summary);
//! # Ok(())
//! # }
//! ```
//!
//! ## Modules
//!
//! - [`ai`] - AI integration (`OpenRouter` API, triage analysis)
//! - [`config`] - Configuration loading and paths
//! - [`error`] - Error types
//! - [`github`] - GitHub API (auth, issues, GraphQL)
//! - [`history`] - Contribution history tracking
//! - [`repos`] - Curated repository list

// ============================================================================
// Error Handling
// ============================================================================

pub use error::AptuError;

/// Convenience Result type for Aptu operations.
///
/// This is equivalent to `std::result::Result<T, AptuError>`.
pub type Result<T> = std::result::Result<T, AptuError>;

// ============================================================================
// Configuration
// ============================================================================

pub use config::{
    AiConfig, AppConfig, CacheConfig, GitHubConfig, UiConfig, UserConfig, config_dir,
    config_file_path, data_dir, load_config,
};

// ============================================================================
// AI Triage
// ============================================================================

pub use ai::openrouter::analyze_issue;
pub use ai::types::{IssueComment, IssueDetails, TriageResponse};

// ============================================================================
// GitHub Integration
// ============================================================================

pub use github::auth::TokenSource;
pub use github::graphql::IssueNode;

// ============================================================================
// History Tracking
// ============================================================================

pub use history::{Contribution, ContributionStatus, HistoryData};

// ============================================================================
// Repository Discovery
// ============================================================================

pub use repos::CuratedRepo;

// ============================================================================
// Modules
// ============================================================================

pub mod ai;
pub mod config;
pub mod error;
pub mod github;
pub mod history;
pub mod repos;
