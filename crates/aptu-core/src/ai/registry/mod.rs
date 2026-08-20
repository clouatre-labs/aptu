// SPDX-License-Identifier: Apache-2.0

//! Centralized provider configuration registry.
//!
//! This module provides a static registry of all AI providers supported by Aptu,
//! including their metadata, API endpoints, and available models.
//!
//! It also provides runtime model validation infrastructure via the `ModelRegistry` trait
//! with a simple sync implementation using static model lists.
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
//! assert_eq!(providers.len(), 7);
//! ```

pub mod config;
pub mod consts;
pub mod parsing;

// Re-export provider constants
pub use consts::{
    PROVIDER_ANTHROPIC, PROVIDER_CEREBRAS, PROVIDER_GEMINI, PROVIDER_GROQ, PROVIDER_OPENROUTER,
    PROVIDER_ZAI, PROVIDER_ZENMUX,
};

// Re-export configuration items
pub use config::{PROVIDERS, ProviderConfig, all_providers, get_provider};

// Re-export parsing and model types
pub use parsing::{CachedModel, Capability, ModelRegistry, PricingInfo, RegistryError};

#[cfg(not(target_arch = "wasm32"))]
pub use parsing::CachedModelRegistry;
