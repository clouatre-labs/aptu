// SPDX-License-Identifier: Apache-2.0

#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

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
    AiConfig, AppConfig, CacheConfig, GitHubConfig, TaskType, UiConfig, UserConfig, config_dir,
    config_file_path, data_dir, load_config, prompts_dir,
};

// ============================================================================
// Caching
// ============================================================================

pub use cache::{CacheEntry, FileCache, FileCacheImpl};

// ============================================================================
// AI Triage
// ============================================================================

pub use ai::types::{
    IssueComment, IssueDetails, PrDetails, PrFile, PrReviewResponse, ReviewEvent, TriageResponse,
};
pub use ai::{AiClient, AiModel, ModelProvider, ProviderConfig, all_providers, get_provider};

// ============================================================================
// GitHub Integration
// ============================================================================

pub use github::auth::TokenSource;
pub use github::graphql::IssueNode;
pub use github::ratelimit::{RateLimitStatus, check_rate_limit};
pub use octocrab::params::State;

// ============================================================================
// AI Integration
// ============================================================================

pub use ai::types::CreditsStatus;

// ============================================================================
// History Tracking
// ============================================================================

pub use history::{AiStats, Contribution, ContributionStatus, HistoryData};

// ============================================================================
// Repository Discovery
// ============================================================================

pub use repos::discovery::{DiscoveredRepo, DiscoveryFilter, search_repositories};
pub use repos::{CuratedRepo, RepoFilter};

// ============================================================================
// Triage Detection
// ============================================================================

pub use triage::{
    APTU_SIGNATURE, TriageStatus, check_already_triaged, render_pr_review_comment_body,
    render_pr_review_markdown, render_triage_markdown,
};

// ============================================================================
// Retry Logic
// ============================================================================

pub use retry::{is_retryable_anyhow, is_retryable_http, retry_backoff};

// ============================================================================
// Bulk Processing
// ============================================================================

pub use bulk::{BulkOutcome, BulkResult, process_bulk};

// ============================================================================
// Utilities
// ============================================================================

pub use utils::{
    format_relative_time, infer_repo_from_git, is_priority_label, parse_and_format_relative_time,
    truncate, truncate_with_suffix,
};

// ============================================================================
// Platform-Agnostic Facade
// ============================================================================

pub use facade::{
    add_custom_repo, analyze_issue, analyze_pr, apply_triage_labels, create_pr, discover_repos,
    fetch_issue_for_triage, fetch_issues, fetch_pr_for_review, format_issue, label_pr,
    list_curated_repos, list_models, list_repos, post_issue, post_pr_review, post_triage_comment,
    remove_custom_repo, validate_model,
};
pub use github::issues::ApplyResult;
pub use github::pulls::PrCreateResult;

// ============================================================================
// Security Scanning
// ============================================================================

pub use security::{
    Confidence, Finding, FindingCache, PatternEngine, SarifReport, SecurityConfig, SecurityScanner,
    Severity, needs_security_scan,
};

// ============================================================================
// Modules
// ============================================================================

pub mod ai;
#[cfg(feature = "ast-context")]
pub mod ast_context;
pub mod auth;
pub mod bulk;
pub mod cache;
pub mod config;
pub mod error;
pub mod facade;
/// Git utilities: patch application, branch management, and version gating.
pub mod git;
pub mod github;
pub mod history;
pub mod repos;
pub mod retry;
pub mod security;
pub mod triage;
pub mod utils;

pub use git::patch::{PatchError, PatchStep, apply_patch_and_push};
