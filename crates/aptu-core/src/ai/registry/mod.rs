// SPDX-License-Identifier: Apache-2.0

//! Centralized provider configuration registry.
//!
//! This module provides a static registry of all AI providers supported by Aptu,
//! including their metadata and API endpoints.
//!
//! # Examples
//!
//! ```
//! use aptu_core::ai::registry::{get_provider, all_providers};
//!
//! // Get a specific provider
//! let provider = get_provider("openrouter");
//! assert!(provider.is_some());
//!
//! // Get all providers
//! let providers = all_providers();
//! assert_eq!(providers.len(), 4);
//! ```

pub mod config;
pub mod consts;

// Re-export provider constants
pub use consts::{PROVIDER_ANTHROPIC, PROVIDER_GEMINI, PROVIDER_OPENROUTER, PROVIDER_ZAI};

// Re-export configuration items
pub use config::{PROVIDERS, ProviderConfig, all_providers, get_provider};
