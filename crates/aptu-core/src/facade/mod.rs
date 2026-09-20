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
pub mod pr_review;

#[cfg(not(target_arch = "wasm32"))]
pub use issues::{analyze_issue, apply_triage_labels, fetch_issue_for_triage, post_triage_comment};
#[cfg(not(target_arch = "wasm32"))]
pub use pr_review::{analyze_pr, fetch_pr_for_review, label_pr, post_pr_review};
