// SPDX-License-Identifier: Apache-2.0

//! Platform-agnostic facade functions for FFI and CLI integration.
//!
//! This module provides high-level functions that abstract away the complexity
//! of credential resolution, API client creation, and data transformation.
//! Each platform (CLI, iOS, MCP) implements `TokenProvider` and calls these
//! functions with their own credential source.

/// Returns `Err(AptuError::GitHub)` with a "not supported on wasm32" message.
///
/// Used in `#[cfg(target_arch = "wasm32")]` stub bodies to avoid repeating
/// the same boilerplate across all facade submodules.
#[cfg(target_arch = "wasm32")]
macro_rules! wasm_unsupported {
    ($fn_name:expr) => {
        return Err(crate::error::AptuError::GitHub {
            message: format!("{} is not supported on wasm32-unknown-unknown", $fn_name),
        })
    };
}

#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_unsupported;

pub mod ai_client;
pub mod issues;
pub mod models;
pub mod pr_create;
pub mod pr_review;
pub mod repos;
pub mod revert;

pub use issues::format_issue;
#[cfg(not(target_arch = "wasm32"))]
pub use issues::{
    analyze_issue, apply_triage_labels, fetch_issue_for_triage, post_issue, post_triage_comment,
};
#[cfg(not(target_arch = "wasm32"))]
pub use models::{list_models, validate_model};
#[cfg(not(target_arch = "wasm32"))]
pub use pr_create::create_pr;
#[cfg(not(target_arch = "wasm32"))]
pub use pr_review::{analyze_pr, fetch_pr_for_review, label_pr, post_pr_review};
#[cfg(not(target_arch = "wasm32"))]
pub use repos::{
    add_custom_repo, discover_repos, fetch_issues, list_curated_repos, list_repos,
    remove_custom_repo,
};
pub use revert::RevertOutcome;
#[cfg(not(target_arch = "wasm32"))]
pub use revert::{revert_issue, revert_pr};
