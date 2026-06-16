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

#[cfg(not(target_arch = "wasm32"))]
pub use config::TomlConfigSource;
#[cfg(not(target_arch = "wasm32"))]
pub use config::load_config;
pub use config::{
    AiConfig, AppConfig, CacheConfig, ConfigSource, GitHubConfig, InMemoryConfigSource, TaskType,
    UiConfig, UserConfig, config_dir, config_file_path, data_dir, prompts_dir,
};

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
#[cfg(not(target_arch = "wasm32"))]
pub use github::ratelimit::check_rate_limit;
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
pub use repos::discovery::search_repositories;
pub use repos::discovery::{DiscoveredRepo, DiscoveryFilter};
pub use repos::{CuratedRepo, RepoFilter};

// ============================================================================
// Triage Detection
// ============================================================================

pub use triage::{
    APTU_SIGNATURE, TriageStatus, check_already_triaged, render_pr_review_comment_body,
    render_pr_review_markdown, render_triage_markdown,
};

// ============================================================================
// Bulk Processing
// ============================================================================

pub use bulk::{BulkOutcome, BulkResult, process_bulk};

// ============================================================================
// Utilities
// ============================================================================

pub use utils::{
    format_relative_time, infer_repo_from_git, parse_and_format_relative_time, truncate,
};

// ============================================================================
// Platform-Agnostic Facade
// ============================================================================

pub use facade::format_issue;
#[cfg(not(target_arch = "wasm32"))]
pub use facade::{
    add_custom_repo, analyze_issue, analyze_pr, apply_triage_labels, create_pr, discover_repos,
    fetch_issue_for_triage, fetch_issues, fetch_pr_for_review, label_pr, list_curated_repos,
    list_models, list_repos, post_issue, post_pr_review, post_triage_comment, remove_custom_repo,
    revert_issue, revert_pr, validate_model,
};
#[cfg(not(target_arch = "wasm32"))]
pub use github::issues::ApplyResult;
#[cfg(not(target_arch = "wasm32"))]
pub use github::pulls::PrCreateResult;

// ============================================================================
// Security Scanning
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub use security::FindingCache;
pub use security::{
    Confidence, Finding, PatternEngine, SarifReport, SecurityConfig, SecurityScanner, Severity,
    needs_security_scan,
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
pub mod metrics;
pub mod repos;
pub mod retry;
pub mod sanitize;
pub mod security;
pub mod triage;
pub mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub use git::patch::{PatchError, PatchStep, apply_patch_and_push};
