// SPDX-License-Identifier: Apache-2.0

#![warn(missing_docs)]

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
//! use aptu_core::{load_config, AiClient, IssueDetails, ai::AiProvider};
//! use anyhow::Result;
//!
//! # async fn example() -> Result<()> {
//! // Load configuration
//! let config = load_config()?;
//!
//! // Create AI client (reuse for multiple requests)
//! let client = AiClient::new(&config.ai.provider, &config.ai)?;
//!
//! // Create issue details
//! let issue = IssueDetails::builder()
//!     .owner("block".to_string())
//!     .repo("goose".to_string())
//!     .number(123)
//!     .title("Example issue".to_string())
//!     .body("Issue description...".to_string())
//!     .labels(vec![])
//!     .comments(vec![])
//!     .url("https://github.com/block/goose/issues/123".to_string())
//!     .build();
//!
//! // Analyze with AI
//! let ai_response = client.analyze_issue(&issue).await?;
//! println!("Summary: {}", ai_response.triage.summary);
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
// Authentication
// ============================================================================

pub use auth::TokenProvider;

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
// Caching
// ============================================================================

pub use cache::CacheEntry;

// ============================================================================
// AI Triage
// ============================================================================

pub use ai::types::{
    IssueComment, IssueDetails, PrDetails, PrFile, PrReviewResponse, ReviewEvent, TriageResponse,
};
pub use ai::{
    AiClient, AiModel, ModelInfo, ModelProvider, ProviderConfig, all_providers, get_provider,
};

// ============================================================================
// GitHub Integration
// ============================================================================

pub use github::auth::TokenSource;
pub use github::graphql::IssueNode;
pub use github::ratelimit::{RateLimitStatus, check_rate_limit};

// ============================================================================
// AI Integration
// ============================================================================

pub use ai::types::CreditsStatus;

// ============================================================================
// History Tracking
// ============================================================================

pub use history::{Contribution, ContributionStatus, HistoryData};

// ============================================================================
// Repository Discovery
// ============================================================================

pub use repos::{CuratedRepo, RepoFilter};

// ============================================================================
// Triage Detection
// ============================================================================

pub use triage::{APTU_SIGNATURE, TriageStatus, check_already_triaged};

// ============================================================================
// Retry Logic
// ============================================================================

pub use retry::{is_retryable_anyhow, is_retryable_http, retry_backoff};

// ============================================================================
// Utilities
// ============================================================================

pub use utils::{
    format_relative_time, parse_and_format_relative_time, truncate, truncate_with_suffix,
};

// ============================================================================
// Platform-Agnostic Facade
// ============================================================================

pub use facade::{
    add_custom_repo, analyze_issue, fetch_issues, list_curated_repos, list_repos, post_pr_review,
    remove_custom_repo, review_pr,
};

// ============================================================================
// Modules
// ============================================================================

pub mod ai;
pub mod auth;
pub mod cache;
pub mod config;
pub mod error;
pub mod facade;
pub mod github;
pub mod history;
pub mod repos;
pub mod retry;
pub mod triage;
pub mod utils;
