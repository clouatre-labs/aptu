// SPDX-License-Identifier: Apache-2.0

//! Provider name constants used throughout the registry.

/// Provider name constant for Anthropic.
///
/// Used throughout the codebase to avoid hardcoding the string literal
/// in multiple places. Replaces all direct "anthropic" comparisons.
pub const PROVIDER_ANTHROPIC: &str = "anthropic";

/// Provider name constant for `OpenRouter`.
pub const PROVIDER_OPENROUTER: &str = "openrouter";

/// Provider name constant for Google Gemini.
pub const PROVIDER_GEMINI: &str = "gemini";

/// Provider name constant for Groq.
pub const PROVIDER_GROQ: &str = "groq";

/// Provider name constant for Cerebras.
pub const PROVIDER_CEREBRAS: &str = "cerebras";

/// Provider name constant for Zenmux.
pub const PROVIDER_ZENMUX: &str = "zenmux";

/// Provider name constant for Z.AI (Zhipu).
pub const PROVIDER_ZAI: &str = "zai";
