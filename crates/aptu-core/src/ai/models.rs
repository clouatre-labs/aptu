// SPDX-License-Identifier: Apache-2.0

//! AI model registry and provider abstraction.
//!
//! This module provides a static registry of available AI models across multiple providers
//! (`OpenRouter`, `Ollama`, `Mlx`). It enables:
//! - Model discovery and filtering by provider
//! - Default model selection for free tier
//! - Model lookup by identifier for configuration validation
//! - Extensibility for future providers
//!
//! # Examples
//!
//! ```
//! use aptu_core::ai::models::{AiModel, ModelProvider};
//!
//! // Get all available models
//! let models = AiModel::available_models();
//! assert!(!models.is_empty());
//!
//! // Get default free model
//! let default = AiModel::default_free();
//! assert!(default.is_free);
//!
//! // Filter by provider
//! let openrouter_models = AiModel::for_provider(ModelProvider::OpenRouter);
//! assert!(!openrouter_models.is_empty());
//!
//! // Find model by identifier
//! let model = AiModel::find_by_identifier("mistralai/devstral-2512:free");
//! assert!(model.is_some());
//! ```

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

impl AiModel {
    /// Returns all available AI models across all providers.
    ///
    /// This is the authoritative registry of models that Aptu supports.
    /// Models are organized by provider and tier (free/paid).
    ///
    /// # Returns
    ///
    /// A vector of all available models, sorted by provider and tier.
    #[must_use]
    pub fn available_models() -> Vec<AiModel> {
        vec![
            // ================================================================
            // OpenRouter - Free Tier Models
            // ================================================================
            AiModel {
                display_name: "Devstral 2".to_string(),
                identifier: "mistralai/devstral-2512:free".to_string(),
                provider: ModelProvider::OpenRouter,
                is_free: true,
                context_window: 262_000,
            },
            AiModel {
                display_name: "Mistral Small 3.1".to_string(),
                identifier: "mistralai/mistral-small-3.1-24b-instruct:free".to_string(),
                provider: ModelProvider::OpenRouter,
                is_free: true,
                context_window: 128_000,
            },
            // ================================================================
            // OpenRouter - Paid Tier Models
            // ================================================================
            AiModel {
                display_name: "Grok Code Fast".to_string(),
                identifier: "x-ai/grok-code-fast-1".to_string(),
                provider: ModelProvider::OpenRouter,
                is_free: false,
                context_window: 256_000,
            },
            AiModel {
                display_name: "Claude Sonnet 4.5".to_string(),
                identifier: "anthropic/claude-sonnet-4.5".to_string(),
                provider: ModelProvider::OpenRouter,
                is_free: false,
                context_window: 1_000_000,
            },
            // ================================================================
            // Ollama - Local Models
            // ================================================================
            AiModel {
                display_name: "Mistral 7B (Local)".to_string(),
                identifier: "mistral:7b".to_string(),
                provider: ModelProvider::Ollama,
                is_free: true,
                context_window: 32_000,
            },
        ]
    }

    /// Returns the default free model for new users.
    ///
    /// Selects the first free `OpenRouter` model from the registry.
    /// This is the recommended starting point for users without API keys.
    ///
    /// # Panics
    ///
    /// Panics if no free `OpenRouter` models are available in the registry.
    /// This should never happen in practice as the registry is hardcoded.
    ///
    /// # Returns
    ///
    /// The default free model (Devstral 2).
    #[must_use]
    pub fn default_free() -> AiModel {
        Self::available_models()
            .into_iter()
            .find(|m| m.is_free && m.provider == ModelProvider::OpenRouter)
            .expect("Registry must contain at least one free OpenRouter model")
    }

    /// Filters models by provider.
    ///
    /// Returns all models from a specific provider, useful for UI dropdowns
    /// or provider-specific configuration.
    ///
    /// # Arguments
    ///
    /// * `provider` - The provider to filter by
    ///
    /// # Returns
    ///
    /// A vector of models from the specified provider, or empty if none exist.
    #[must_use]
    pub fn for_provider(provider: ModelProvider) -> Vec<AiModel> {
        Self::available_models()
            .into_iter()
            .filter(|m| m.provider == provider)
            .collect()
    }

    /// Finds a model by its identifier.
    ///
    /// Used for configuration validation and model lookup from user input.
    /// Identifiers are case-sensitive and must match exactly.
    ///
    /// # Arguments
    ///
    /// * `identifier` - The model identifier to search for
    ///
    /// # Returns
    ///
    /// Some(model) if found, None otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use aptu_core::ai::models::AiModel;
    ///
    /// let model = AiModel::find_by_identifier("mistralai/devstral-2512:free");
    /// assert!(model.is_some());
    /// assert_eq!(model.unwrap().display_name, "Devstral 2");
    /// ```
    #[must_use]
    pub fn find_by_identifier(identifier: &str) -> Option<AiModel> {
        Self::available_models()
            .into_iter()
            .find(|m| m.identifier == identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_models_not_empty() {
        let models = AiModel::available_models();
        assert!(
            !models.is_empty(),
            "Registry must contain at least one model"
        );
    }

    #[test]
    fn test_available_models_have_unique_identifiers() {
        let models = AiModel::available_models();
        let mut identifiers = Vec::new();
        for model in &models {
            assert!(
                !identifiers.contains(&model.identifier),
                "Duplicate identifier: {}",
                model.identifier
            );
            identifiers.push(model.identifier.clone());
        }
    }

    #[test]
    fn test_default_free_is_free() {
        let model = AiModel::default_free();
        assert!(model.is_free, "Default model must be free");
    }

    #[test]
    fn test_default_free_is_openrouter() {
        let model = AiModel::default_free();
        assert_eq!(
            model.provider,
            ModelProvider::OpenRouter,
            "Default model must be from OpenRouter"
        );
    }

    #[test]
    fn test_for_provider_openrouter() {
        let models = AiModel::for_provider(ModelProvider::OpenRouter);
        assert!(!models.is_empty(), "OpenRouter should have models");
        assert!(
            models
                .iter()
                .all(|m| m.provider == ModelProvider::OpenRouter),
            "All returned models should be from OpenRouter"
        );
    }

    #[test]
    fn test_for_provider_ollama() {
        let models = AiModel::for_provider(ModelProvider::Ollama);
        assert!(!models.is_empty(), "Ollama should have models");
        assert!(
            models.iter().all(|m| m.provider == ModelProvider::Ollama),
            "All returned models should be from Ollama"
        );
    }

    #[test]
    fn test_for_provider_mlx_empty() {
        let models = AiModel::for_provider(ModelProvider::Mlx);
        assert!(
            models.is_empty(),
            "MLX should have no models in Phase 1 (reserved for future)"
        );
    }

    #[test]
    fn test_find_by_identifier_devstral() {
        let model = AiModel::find_by_identifier("mistralai/devstral-2512:free");
        assert!(model.is_some(), "Should find Devstral model");
        let model = model.unwrap();
        assert_eq!(model.display_name, "Devstral 2");
        assert!(model.is_free);
    }

    #[test]
    fn test_find_by_identifier_claude() {
        let model = AiModel::find_by_identifier("anthropic/claude-sonnet-4.5");
        assert!(model.is_some(), "Should find Claude model");
        let model = model.unwrap();
        assert_eq!(model.display_name, "Claude Sonnet 4.5");
        assert!(!model.is_free);
    }

    #[test]
    fn test_find_by_identifier_not_found() {
        let model = AiModel::find_by_identifier("nonexistent/model");
        assert!(model.is_none(), "Should not find nonexistent model");
    }

    #[test]
    fn test_find_by_identifier_case_sensitive() {
        let model = AiModel::find_by_identifier("MISTRALAI/DEVSTRAL-2512:FREE");
        assert!(
            model.is_none(),
            "Identifier lookup should be case-sensitive"
        );
    }

    #[test]
    fn test_model_provider_display() {
        assert_eq!(ModelProvider::OpenRouter.to_string(), "OpenRouter");
        assert_eq!(ModelProvider::Ollama.to_string(), "Ollama");
        assert_eq!(ModelProvider::Mlx.to_string(), "MLX");
    }

    #[test]
    fn test_free_models_have_reasonable_context() {
        let free_models = AiModel::available_models()
            .into_iter()
            .filter(|m| m.is_free)
            .collect::<Vec<_>>();

        assert!(!free_models.is_empty(), "Should have free models");
        for model in free_models {
            assert!(
                model.context_window >= 32_000,
                "Free model {} should have at least 32K context",
                model.display_name
            );
        }
    }

    #[test]
    fn test_paid_models_have_larger_context() {
        let paid_models = AiModel::available_models()
            .into_iter()
            .filter(|m| !m.is_free)
            .collect::<Vec<_>>();

        assert!(!paid_models.is_empty(), "Should have paid models");
        for model in paid_models {
            assert!(
                model.context_window >= 256_000,
                "Paid model {} should have at least 256K context",
                model.display_name
            );
        }
    }

    #[test]
    fn test_model_serialization() {
        let model = AiModel::default_free();
        let json = serde_json::to_string(&model).expect("Should serialize");
        let deserialized: AiModel = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(model, deserialized);
    }

    #[test]
    fn test_model_provider_serialization() {
        let provider = ModelProvider::OpenRouter;
        let json = serde_json::to_string(&provider).expect("Should serialize");
        let deserialized: ModelProvider = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(provider, deserialized);
    }
}
