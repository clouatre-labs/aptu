// SPDX-License-Identifier: Apache-2.0

//! AI model and provider types.
//!
//! This module provides core types for AI model and provider representation.
//! Runtime model validation is handled by the `ModelRegistry` trait in the registry module.

use serde::{Deserialize, Serialize};

/// AI provider identifier.
///
/// Represents different AI service providers that Aptu can integrate with.
/// Each provider has different capabilities, pricing, and deployment models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelProvider {
    /// `OpenRouter` - Unified API for multiple AI providers
    /// Supports free and paid models from Mistral, Anthropic, xAI, and others.
    OpenRouter,

    /// `Ollama` - Local AI model runner
    /// Runs models locally without API calls or costs.
    Ollama,

    /// `MLX` - Apple Silicon optimized models (future iOS support)
    /// Runs models natively on iOS devices.
    Mlx,
}

impl std::fmt::Display for ModelProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelProvider::OpenRouter => write!(f, "OpenRouter"),
            ModelProvider::Ollama => write!(f, "Ollama"),
            ModelProvider::Mlx => write!(f, "MLX"),
        }
    }
}

/// AI model metadata and configuration.
///
/// Represents a single AI model with its capabilities, pricing, and provider information.
/// Used for model selection, validation, and UI display.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiModel {
    /// Human-readable model name for UI display
    /// Example: "Devstral 2", "Claude Sonnet 4.5"
    pub display_name: String,

    /// Provider-specific model identifier
    /// Used in API requests to specify which model to use.
    /// Examples:
    /// - `OpenRouter`: "mistralai/devstral-2512:free"
    /// - `Ollama`: "mistral:7b"
    pub identifier: String,

    /// AI service provider
    pub provider: ModelProvider,

    /// Whether this model is free to use
    /// Free models have no API cost (either free tier or local execution).
    pub is_free: bool,

    /// Maximum context window size in tokens
    /// Determines how much input text the model can process.
    pub context_window: u32,
}
